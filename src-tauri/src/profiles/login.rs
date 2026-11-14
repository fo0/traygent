//! The hidden sign-in of a profile (#41, „Debug“ off): the plan of
//! [`crate::launch::hidden_login_plan`] runs without a window, stdin closed, for at most
//! [`LOGIN_LIMIT`]. One sign-in per profile at a time; a cancel, the limit or the app's exit
//! ends the whole process tree.
//!
//! stdout and stderr go to the null device: the CLI's output can hold the sign-in URL or a
//! code, so it never reaches Traygent's memory, a log or the panel.
use super::status::Key;
use crate::launch::HiddenLogin;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// How long a hidden sign-in may take before it is ended.
pub const LOGIN_LIMIT: Duration = Duration::from_secs(5 * 60);
/// How long the app's exit waits for running sign-ins to end their process trees.
pub const EXIT_WAIT: Duration = Duration::from_secs(3);
pub const LOGIN_FAILED: &str = "Anmeldung fehlgeschlagen.";
pub const LOGIN_TIMEOUT: &str = "Anmeldung abgelaufen.";
pub const LOGIN_CANCELLED: &str = "Anmeldung abgebrochen.";
pub const LOGIN_RUNNING: &str = "Die Anmeldung läuft bereits.";
const POLL: Duration = Duration::from_millis(50);

/// How a hidden sign-in ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoginEnd {
    /// The process exited; `None` without a code (ended by a signal).
    Exited(Option<i32>),
    TimedOut,
    Cancelled,
    /// The process could not be started.
    NotStarted,
}

/// Only exit code 0 is a sign-in; everything else is the matching German message.
pub fn verdict(end: &LoginEnd) -> Result<(), &'static str> {
    match end {
        LoginEnd::Exited(Some(0)) => Ok(()),
        LoginEnd::Exited(_) | LoginEnd::NotStarted => Err(LOGIN_FAILED),
        LoginEnd::TimedOut => Err(LOGIN_TIMEOUT),
        LoginEnd::Cancelled => Err(LOGIN_CANCELLED),
    }
}

/// Runs a hidden sign-in; injected so the decision logic is tested without a CLI.
pub trait LoginRunner: Sync {
    fn run(&self, plan: &HiddenLogin, cancel: &AtomicBool, limit: Duration) -> LoginEnd;
}

/// The real process: [`run_hidden`].
pub struct ProcessLoginRunner;

impl LoginRunner for ProcessLoginRunner {
    fn run(&self, plan: &HiddenLogin, cancel: &AtomicBool, limit: Duration) -> LoginEnd {
        run_hidden(
            Path::new(&plan.program),
            &plan.args,
            &plan.cwd,
            cancel,
            limit,
        )
    }
}

/// One process without a window: absolute program only, stdin closed, stdout and stderr to
/// the null device. Polls for its end first, then `cancel` and `limit`; the last two end the
/// process tree. A process that ended in the same tick as a cancel counts as ended.
pub fn run_hidden(
    program: &Path,
    args: &[String],
    cwd: &str,
    cancel: &AtomicBool,
    limit: Duration,
) -> LoginEnd {
    if !program.is_absolute() {
        return LoginEnd::NotStarted;
    }
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let Ok(mut child) = command.spawn() else {
        return LoginEnd::NotStarted;
    };
    let deadline = Instant::now() + limit;
    let poll = |child: &mut Child| child.try_wait().map(|status| status.map(|s| s.code()));
    watch(&mut child, poll, stop, cancel, deadline)
}

/// The wait loop of [`run_hidden`]: `poll` first — a process that ended in the same tick as
/// a cancel or the limit counts as ended —, then `cancel`, then `deadline`, which `stop` the
/// process.
fn watch<P>(
    process: &mut P,
    mut poll: impl FnMut(&mut P) -> std::io::Result<Option<Option<i32>>>,
    mut stop: impl FnMut(&mut P),
    cancel: &AtomicBool,
    deadline: Instant,
) -> LoginEnd {
    loop {
        match poll(process) {
            Ok(Some(code)) => return LoginEnd::Exited(code),
            Ok(None) if cancel.load(Ordering::SeqCst) => {
                stop(process);
                return LoginEnd::Cancelled;
            }
            Ok(None) if Instant::now() >= deadline => {
                stop(process);
                return LoginEnd::TimedOut;
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(_) => {
                stop(process);
                return LoginEnd::Exited(None);
            }
        }
    }
}

/// Ends the process and everything it started (the CLI under PowerShell): the tree through
/// [`crate::detect::runner::kill_tree`] on Windows (absolute `taskkill`, bounded wait), then
/// the process itself.
fn stop(child: &mut Child) {
    #[cfg(windows)]
    crate::detect::runner::kill_tree(child.id());
    let _ = child.kill();
    let _ = child.wait();
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

static RUNNING: Mutex<Vec<(Key, Arc<AtomicBool>)>> = Mutex::new(Vec::new());

/// The registration of a running sign-in; dropping it (also while unwinding) frees the
/// profile.
pub struct Ticket {
    cancel: Arc<AtomicBool>,
}

/// Registers a sign-in of `key`; refused while one runs for it.
pub fn begin(key: Key) -> Result<Ticket, &'static str> {
    let mut running = lock(&RUNNING);
    if running.iter().any(|(listed, _)| *listed == key) {
        return Err(LOGIN_RUNNING);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    running.push((key, Arc::clone(&cancel)));
    Ok(Ticket { cancel })
}

impl Drop for Ticket {
    fn drop(&mut self) {
        lock(&RUNNING).retain(|(_, flag)| !Arc::ptr_eq(flag, &self.cancel));
    }
}

/// Asks the running sign-in of `key` to end; false when none is registered.
pub fn cancel(key: &Key) -> bool {
    let running = lock(&RUNNING);
    let found = running.iter().find(|(listed, _)| listed == key);
    if let Some((_, flag)) = found {
        flag.store(true, Ordering::SeqCst);
    }
    found.is_some()
}

/// Whether a sign-in of `key` is registered.
pub fn running(key: &Key) -> bool {
    lock(&RUNNING).iter().any(|(listed, _)| listed == key)
}

/// The app exits: asks every running sign-in to end and waits at most `wait` for their
/// process trees (best effort). True when none is left.
pub fn end_all(wait: Duration) -> bool {
    for (_, flag) in lock(&RUNNING).iter() {
        flag.store(true, Ordering::SeqCst);
    }
    let deadline = Instant::now() + wait;
    loop {
        if lock(&RUNNING).is_empty() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Runs the ticket's sign-in to its end: `Ok` only after exit code 0.
pub fn sign_in(
    runner: &dyn LoginRunner,
    plan: &HiddenLogin,
    ticket: &Ticket,
) -> Result<(), String> {
    // Cancelled while it was being prepared: nothing starts.
    if ticket.cancel.load(Ordering::SeqCst) {
        return Err(LOGIN_CANCELLED.into());
    }
    let end = runner.run(plan, &ticket.cancel, LOGIN_LIMIT);
    verdict(&end).map_err(String::from)
}

#[cfg(test)]
mod tests;
