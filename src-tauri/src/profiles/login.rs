//! The hidden sign-in of a profile (#41, „Debug“ off): the plan of
//! [`crate::launch::hidden_login_plan`] runs without a window, stdin closed, stdout and
//! stderr drained into one buffer capped at [`OUTPUT_LIMIT`], for at most [`LOGIN_LIMIT`].
//! One sign-in per profile at a time; a cancel or the limit ends the whole process tree.
//!
//! The output is never logged, persisted or shown: it can hold the sign-in URL or a code.
use super::status::Key;
use crate::launch::HiddenLogin;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// How long a hidden sign-in may take before it is ended.
pub const LOGIN_LIMIT: Duration = Duration::from_secs(5 * 60);
/// What is kept of stdout and stderr together; the rest is drained and dropped.
pub const OUTPUT_LIMIT: usize = 64 * 1024;
pub const LOGIN_FAILED: &str = "Anmeldung fehlgeschlagen.";
pub const LOGIN_TIMEOUT: &str = "Anmeldung abgelaufen.";
pub const LOGIN_CANCELLED: &str = "Anmeldung abgebrochen.";
pub const LOGIN_RUNNING: &str = "Die Anmeldung läuft bereits.";
const POLL: Duration = Duration::from_millis(50);
/// How long the streams may still deliver after the process ended.
const DRAIN: Duration = Duration::from_millis(500);

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

/// The captured output. Deliberately without `Debug`; wiped on drop.
#[derive(Default)]
pub struct Output {
    bytes: Vec<u8>,
}

impl Output {
    /// The text without ANSI escape sequences.
    pub fn text(&self) -> String {
        strip_ansi(&String::from_utf8_lossy(&self.bytes))
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        crate::github::secret::wipe(&mut self.bytes);
    }
}

/// CSI (`ESC [ … final`), OSC (`ESC ] … BEL` or `ESC ] … ESC \`) and two-character escapes
/// removed.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// A finished hidden sign-in.
pub struct LoginRun {
    pub end: LoginEnd,
    pub output: Output,
}

/// Runs a hidden sign-in; injected so the decision logic is tested without a CLI.
pub trait LoginRunner: Sync {
    fn run(&self, plan: &HiddenLogin, cancel: &AtomicBool, limit: Duration) -> LoginRun;
}

/// The real process: [`run_hidden`].
pub struct ProcessLoginRunner;

impl LoginRunner for ProcessLoginRunner {
    fn run(&self, plan: &HiddenLogin, cancel: &AtomicBool, limit: Duration) -> LoginRun {
        run_hidden(
            Path::new(&plan.program),
            &plan.args,
            &plan.cwd,
            cancel,
            limit,
        )
    }
}

/// One process without a window: absolute program only, stdin closed, both streams into one
/// capped buffer. Polls for its end, `cancel` and `limit`; the last two end the process tree.
pub fn run_hidden(
    program: &Path,
    args: &[String],
    cwd: &str,
    cancel: &AtomicBool,
    limit: Duration,
) -> LoginRun {
    let mut output = Output::default();
    if !program.is_absolute() {
        return LoginRun {
            end: LoginEnd::NotStarted,
            output,
        };
    }
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let Ok(mut child) = command.spawn() else {
        return LoginRun {
            end: LoginEnd::NotStarted,
            output,
        };
    };
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let (send, receive) = mpsc::channel();
    for stream in [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        drain(stream, Arc::clone(&buffer), send.clone());
    }
    drop(send);
    let deadline = Instant::now() + limit;
    let end = loop {
        if cancel.load(Ordering::SeqCst) {
            stop(&mut child);
            break LoginEnd::Cancelled;
        }
        match child.try_wait() {
            Ok(Some(status)) => break LoginEnd::Exited(status.code()),
            Ok(None) if Instant::now() >= deadline => {
                stop(&mut child);
                break LoginEnd::TimedOut;
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(_) => {
                stop(&mut child);
                break LoginEnd::Exited(None);
            }
        }
    };
    // A grandchild (the browser) may hold the pipes open: wait briefly, never for it.
    let until = Instant::now() + DRAIN;
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        if receive.recv_timeout(left).is_err() {
            break;
        }
    }
    output.bytes = std::mem::take(&mut *lock(&buffer));
    LoginRun { end, output }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Reads `stream` to its end; keeps what fits under [`OUTPUT_LIMIT`], drops the rest.
fn drain(mut stream: Box<dyn Read + Send>, buffer: Arc<Mutex<Vec<u8>>>, done: mpsc::Sender<()>) {
    std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(read) = stream.read(&mut chunk) {
            if read == 0 {
                break;
            }
            let mut kept = lock(&buffer);
            let room = OUTPUT_LIMIT.saturating_sub(kept.len()).min(read);
            kept.extend_from_slice(&chunk[..room]);
        }
        chunk.fill(0);
        let _ = done.send(());
    });
}

/// Ends the process and everything it started (the CLI under PowerShell): `taskkill /T` on
/// Windows, then the process itself.
fn stop(child: &mut Child) {
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let taskkill = std::path::PathBuf::from(root).join(r"System32\taskkill.exe");
        let _ = Command::new(taskkill)
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
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

/// Asks the running sign-in of `key` to end; false when none runs.
pub fn cancel(key: &Key) -> bool {
    let running = lock(&RUNNING);
    let found = running.iter().find(|(listed, _)| listed == key);
    if let Some((_, flag)) = found {
        flag.store(true, Ordering::SeqCst);
    }
    found.is_some()
}

/// Whether a sign-in of `key` runs.
pub fn running(key: &Key) -> bool {
    lock(&RUNNING).iter().any(|(listed, _)| listed == key)
}

/// Runs the ticket's sign-in to its end: `Ok` only after exit code 0.
pub fn sign_in(
    runner: &dyn LoginRunner,
    plan: &HiddenLogin,
    ticket: &Ticket,
) -> Result<(), String> {
    let run = runner.run(plan, &ticket.cancel, LOGIN_LIMIT);
    verdict(&run.end).map_err(String::from)
}

#[cfg(test)]
mod tests;
