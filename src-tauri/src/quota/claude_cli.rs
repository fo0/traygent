//! Claude Code's quota through its own CLI (#110, ADR-0015): the hidden
//! `claude -p /usage --output-format json --no-session-persistence` of
//! [`crate::launch::hidden_usage_plan`], started like the hidden sign-in
//! ([`crate::profiles::login::hidden_command`]): no window, stdin closed, stderr to the null
//! device, stdout capped at [`MAX_OUTPUT`], ended with its whole process tree after
//! [`READ_LIMIT`] or at the app's exit ([`end_cli_reads`]).
//!
//! `/usage` prints no sign-in material, so its stdout is read — but never logged: it lists
//! the user's skills, subagents and MCP servers. The log gets the exit, the duration, the
//! number of windows and, at the debug level, the parsed metrics and percentages.
use super::{claude_usage, AgentQuota};
use crate::launch::HiddenLogin;
use crate::models::QuotaMetric;
use crate::profiles::login::{hidden_command, watch_child, LoginEnd};
use chrono::{DateTime, TimeZone};
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// stdout beyond this is no `/usage` answer: the read fails.
pub const MAX_OUTPUT: usize = 64 * 1024;
/// The CLI answers in about 1.5 s; past this its process tree is ended.
pub const READ_LIMIT: Duration = Duration::from_secs(20);
const TIMED_OUT: &str = "Die Claude-Code-CLI hat nicht rechtzeitig geantwortet.";
const NOT_STARTED: &str = "Die Claude-Code-CLI konnte nicht gestartet werden.";
const FAILED: &str = "Die Claude-Code-CLI konnte das Kontingent nicht lesen.";

/// How one CLI run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliEnd {
    /// The process exited (`None`: no code); `stdout` holds at most [`MAX_OUTPUT`] + 1 bytes.
    Exited {
        code: Option<i32>,
        stdout: Vec<u8>,
    },
    TimedOut,
    NotStarted,
}

/// Runs the read's plan; injected so the decisions are tested without a CLI.
pub trait CliRunner: Sync {
    fn run(&self, plan: &HiddenLogin, limit: Duration) -> CliEnd;
}

/// The real process.
pub struct ProcessCliRunner;

impl CliRunner for ProcessCliRunner {
    fn run(&self, plan: &HiddenLogin, limit: Duration) -> CliEnd {
        run_process(plan, &EXITING, limit)
    }
}

/// Set once by [`end_cli_reads`]: the app exits, a running read ends its process tree and no
/// new one starts.
static EXITING: AtomicBool = AtomicBool::new(false);

/// How long the read's stdout may stay open after the shell exited.
const PIPE_WAIT: Duration = Duration::from_millis(500);

/// One run of the plan, ended with its process tree on `cancel` or after `limit`.
fn run_process(plan: &HiddenLogin, cancel: &AtomicBool, limit: Duration) -> CliEnd {
    if cancel.load(Ordering::SeqCst) {
        return CliEnd::NotStarted;
    }
    let Some(mut command) = hidden_command(Path::new(&plan.program), &plan.args, &plan.cwd) else {
        return CliEnd::NotStarted;
    };
    let Ok(mut child) = command.stdout(Stdio::piped()).spawn() else {
        return CliEnd::NotStarted;
    };
    let stdout = child.stdout.take();
    let kept = Arc::new(Mutex::new(Vec::new()));
    let (sender, closed) = mpsc::channel();
    let shared = Arc::clone(&kept);
    std::thread::spawn(move || {
        read_shared(stdout, &shared, MAX_OUTPUT + 1);
        let _ = sender.send(());
    });
    match watch_child(&mut child, cancel, limit) {
        LoginEnd::Exited(code) => {
            // A grandchild (npm shim → node, or one the CLI left behind) may hold the pipe
            // after the shell exited: what it printed so far counts, and what is left of the
            // tree is ended so such processes cannot pile up.
            if closed.recv_timeout(PIPE_WAIT).is_err() {
                #[cfg(windows)]
                crate::detect::runner::kill_tree(child.id());
            }
            let stdout = std::mem::take(&mut *lock(&kept));
            CliEnd::Exited { code, stdout }
        }
        LoginEnd::NotStarted => CliEnd::NotStarted,
        // Cancelled only by the app's exit, where the answer is no longer shown.
        LoginEnd::TimedOut | LoginEnd::Cancelled => CliEnd::TimedOut,
    }
}

/// Appends the first `cap` bytes of `reader` to `kept` as they come and drains the rest, so
/// the child never blocks on a full pipe and the caller can take what came at any time.
fn read_shared(reader: Option<impl Read>, kept: &Mutex<Vec<u8>>, cap: usize) {
    let Some(mut reader) = reader else {
        return;
    };
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(read) => {
                let mut kept = lock(kept);
                let room = cap.saturating_sub(kept.len());
                kept.extend_from_slice(&buffer[..read.min(room)]);
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The app exits (`RunEvent::Exit`, like [`crate::profiles::login::end_all`]): every running
/// read ends its process tree, none starts any more, and this waits at most `wait` for the
/// counted reads to end. True when none is left.
pub fn end_cli_reads(wait: Duration) -> bool {
    EXITING.store(true, Ordering::SeqCst);
    wait_for_cli_reads(wait)
}

static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

/// A read that is about to run or running, counted until it is dropped. The caller begins it
/// under the service lock in which it checked that no update replaces the CLI, so an update
/// that marks the agent as updating afterwards sees it in [`wait_for_cli_reads`].
pub struct CliRead {
    pub plan: HiddenLogin,
    _counted: Counted,
}

struct Counted;

impl Drop for Counted {
    fn drop(&mut self) {
        IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
    }
}

impl CliRead {
    pub fn new(plan: HiddenLogin) -> Self {
        IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
        Self {
            plan,
            _counted: Counted,
        }
    }
}

/// Waits at most `wait` until no CLI read runs; true when none does. The agent update calls
/// it after marking Claude Code as updating, so a read never makes its in-use check skip
/// the update (`agent_updates.rs`).
pub fn wait_for_cli_reads(wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        if IN_FLIGHT.load(Ordering::SeqCst) == 0 {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// One read, logged (#109): a debug line for every read, an info line when it failed (not
/// for a sign-in without a subscription).
/// `now` carries the local zone the CLI prints its reset times in.
pub(super) fn read<Tz: TimeZone>(
    runner: &dyn CliRunner,
    profile: &str,
    read: &CliRead,
    now: &DateTime<Tz>,
) -> AgentQuota {
    let started = Instant::now();
    let end = runner.run(&read.plan, READ_LIMIT);
    let quota = answer(&end, now);
    let (debug, info) = lines(profile, &end, &quota, started.elapsed());
    crate::app_log::debug(|| debug);
    if let Some(info) = info {
        crate::app_log::info(&info);
    }
    quota
}

/// The quota a run's end gives.
fn answer<Tz: TimeZone>(end: &CliEnd, now: &DateTime<Tz>) -> AgentQuota {
    match end {
        CliEnd::TimedOut => AgentQuota::unavailable(TIMED_OUT),
        CliEnd::NotStarted => AgentQuota::unavailable(NOT_STARTED),
        CliEnd::Exited { stdout, .. } if stdout.len() > MAX_OUTPUT => {
            AgentQuota::unavailable(claude_usage::UNREADABLE)
        }
        CliEnd::Exited {
            code: Some(0),
            stdout,
        } => match claude_usage::result_text(stdout) {
            Ok(text) => claude_usage::parse(&text, now),
            Err(reason) => AgentQuota::unavailable(reason),
        },
        CliEnd::Exited { .. } => AgentQuota::unavailable(FAILED),
    }
}

/// `quota claude profile=<id> -> cli exit=<code|signal|timeout|not-started> <ms>ms
/// windows=<n>` plus the parsed `<metric>=<percent>` pairs; the info line when no window came.
fn lines(
    profile: &str,
    end: &CliEnd,
    quota: &AgentQuota,
    took: Duration,
) -> (String, Option<String>) {
    let who = format!("quota claude profile={profile}");
    let exit = match end {
        CliEnd::Exited {
            code: Some(code), ..
        } => code.to_string(),
        CliEnd::Exited { code: None, .. } => "signal".into(),
        CliEnd::TimedOut => "timeout".into(),
        CliEnd::NotStarted => "not-started".into(),
    };
    let mut debug = format!(
        "{who} -> cli exit={exit} {}ms windows={}",
        took.as_millis(),
        quota.windows.len()
    );
    for (metric, window) in &quota.windows {
        debug.push_str(&format!(" {}={}", metric_name(*metric), window.used));
    }
    // A sign-in without a subscription is a lasting state, not a failure: like Codex's
    // signed-out read (`read_log.rs`) it stays at the debug level, with its reason.
    let lasting = quota.detail.as_deref() == Some(claude_usage::NO_SUBSCRIPTION);
    if lasting {
        debug.push_str(&format!(" reason={}", claude_usage::NO_SUBSCRIPTION));
    }
    let info = quota
        .detail
        .as_ref()
        .filter(|_| quota.windows.is_empty() && !lasting)
        .map(|reason| format!("{who} failed: cli exit={exit} reason={reason}"));
    (debug, info)
}

/// The metric's IPC name (`QuotaMetric` serializes in camelCase).
fn metric_name(metric: QuotaMetric) -> &'static str {
    match metric {
        QuotaMetric::Week => "week",
        QuotaMetric::Session => "session",
        QuotaMetric::Fable => "fable",
        QuotaMetric::Sonnet => "sonnet",
        QuotaMetric::Spark => "spark",
        QuotaMetric::SparkWeek => "sparkWeek",
        QuotaMetric::Month => "month",
    }
}

#[cfg(test)]
mod tests;
