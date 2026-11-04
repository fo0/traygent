//! Runs gh: absolute program only, a minimal environment without GitHub token variables,
//! closed stdin, stdout and stderr each capped at 64 KiB (more fails the run), a 20 s
//! timeout, no console window. Output is wiped on drop and never formatted into an error.
//!
//! `detect::SystemRunner` truncates output, inherits the environment and has a 5 s
//! timeout, so token reads get this stricter runner.
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::secret::wipe;

pub const LIMIT: usize = 64 * 1024;
pub const TIMEOUT: Duration = Duration::from_secs(20);
pub const GH_FAILED: &str = "gh konnte nicht geprüft werden. Im Terminal „gh auth status“ prüfen.";
pub const GH_TIMEOUT: &str = "gh antwortet nicht (Zeitlimit erreicht).";

/// A finished run. Deliberately without `Debug`: stdout may hold a token.
pub struct Output {
    pub success: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Drop for Output {
    fn drop(&mut self) {
        wipe(&mut self.stdout);
        wipe(&mut self.stderr);
    }
}

pub trait Runner: Sync {
    /// `Err` is a German message without process output.
    fn run(&self, program: &Path, args: &[&str]) -> Result<Output, String>;
}

pub struct ProcessRunner;

impl Runner for ProcessRunner {
    fn run(&self, program: &Path, args: &[&str]) -> Result<Output, String> {
        run_process(program, args, TIMEOUT)
    }
}

/// The variables a child keeps: the user's profile and config dirs, temp, proxy and CA
/// trust. No GH_TOKEN/GITHUB_TOKEN, GH_HOST, GH_DEBUG, GIT_* or askpass settings.
const KEPT: [&str; 16] = [
    "HOME",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "SystemRoot",
    "WINDIR",
    "TEMP",
    "TMP",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

fn kill(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub(crate) fn run_process(
    program: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<Output, String> {
    if !program.is_absolute() {
        return Err(GH_FAILED.into());
    }
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in KEPT {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    // Honour the user's explicit gh store; never substitute a different one.
    if let Some(directory) = std::env::var_os("GH_CONFIG_DIR") {
        if !Path::new(&directory).is_absolute() {
            return Err("GH_CONFIG_DIR muss ein absoluter Pfad sein.".into());
        }
        command.env("GH_CONFIG_DIR", directory);
    }
    command
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("GH_NO_EXTENSION_UPDATE_NOTIFIER", "1")
        .env("NO_COLOR", "1")
        .env("GCM_INTERACTIVE", "0");
    // Never search Traygent's or a project's directory, even for child utilities.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
        let root = std::env::var_os("SystemRoot").ok_or(GH_FAILED)?;
        let system = std::path::PathBuf::from(root).join("System32");
        command.env("PATH", &system).current_dir(system);
    }
    #[cfg(not(windows))]
    command.env("PATH", "/usr/bin:/bin").current_dir("/");
    let mut child = command.spawn().map_err(|_| GH_FAILED.to_string())?;
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        kill(&mut child);
        return Err(GH_FAILED.into());
    };
    let (send, receive) = mpsc::sync_channel(2);
    capture(stdout, send.clone(), false);
    capture(stderr, send, true);
    let deadline = Instant::now() + timeout;
    // Collected into an `Output` from the start so every early return wipes what arrived.
    let mut output = Output {
        success: false,
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    let (mut out, mut err) = (false, false);
    loop {
        if Instant::now() >= deadline {
            kill(&mut child);
            return Err(GH_TIMEOUT.into());
        }
        match receive.try_recv() {
            Ok((false, Some(bytes))) => (output.stdout, out) = (bytes, true),
            Ok((true, Some(bytes))) => (output.stderr, err) = (bytes, true),
            // A stream over the limit or unreadable fails the run.
            Ok((_, None)) => {
                kill(&mut child);
                return Err(GH_FAILED.into());
            }
            Err(mpsc::TryRecvError::Disconnected) if !out || !err => {
                kill(&mut child);
                return Err(GH_FAILED.into());
            }
            _ => {}
        }
        match child.try_wait() {
            Ok(Some(status)) if out && err => {
                output.success = status.success();
                return Ok(output);
            }
            Err(_) => {
                kill(&mut child);
                return Err(GH_FAILED.into());
            }
            _ => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// Reads one stream up to the limit; `None` when it is longer or unreadable.
fn capture(
    stream: impl Read + Send + 'static,
    send: mpsc::SyncSender<(bool, Option<Vec<u8>>)>,
    stderr: bool,
) {
    std::thread::spawn(move || {
        // Room for the whole capped stream: `read_to_end` never reallocates (and so never
        // leaves an unwiped copy of a token behind).
        let mut bytes = Vec::with_capacity(LIMIT + 1);
        let result = stream.take(LIMIT as u64 + 1).read_to_end(&mut bytes);
        let kept = if result.is_ok() && bytes.len() <= LIMIT {
            Some(bytes)
        } else {
            wipe(&mut bytes);
            None
        };
        // The run already ended (timeout, the other stream failed): wipe what nobody reads.
        if let Err(mpsc::SendError((_, Some(mut bytes)))) = send.send((stderr, kept)) {
            wipe(&mut bytes);
        }
    });
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str, timeout: Duration) -> Result<Output, String> {
        run_process(Path::new("/bin/sh"), &["-c", script], timeout)
    }

    #[test]
    fn children_get_no_token_variables_and_stderr_stays_apart() {
        // Synthetic programs only; never the developer machine's gh.
        std::env::set_var("GH_TOKEN", "synthetic-inherited");
        std::env::set_var("GITHUB_TOKEN", "synthetic-inherited");
        let script =
            "test -z \"$GH_TOKEN$GITHUB_TOKEN$GH_HOST\" && test \"$GH_PROMPT_DISABLED\" = 1 \
                      && printf fixture-output && printf private-diagnostic >&2";
        let out = sh(script, Duration::from_secs(5)).unwrap();
        assert!(out.success);
        assert_eq!(out.stdout, b"fixture-output");
        assert_eq!(out.stderr, b"private-diagnostic");
        assert!(!sh("exit 3", Duration::from_secs(5)).unwrap().success);
    }

    #[test]
    fn output_limits_timeouts_and_relative_programs_fail_without_output() {
        let five = Duration::from_secs(5);
        assert!(sh("head -c 70000 /dev/zero", five).is_err());
        assert!(sh("head -c 70000 /dev/zero >&2", five).is_err());
        assert_eq!(
            sh("head -c 65536 /dev/zero", five).unwrap().stdout.len(),
            LIMIT
        );
        let start = Instant::now();
        let error = sh("sleep 10", Duration::from_millis(100)).err().unwrap();
        assert_eq!(error, GH_TIMEOUT);
        assert!(start.elapsed() < Duration::from_secs(3));
        assert_eq!(
            run_process(Path::new("gh.exe"), &[], TIMEOUT)
                .err()
                .unwrap(),
            GH_FAILED
        );
        assert_eq!(
            run_process(Path::new("/nonexistent/gh"), &[], TIMEOUT)
                .err()
                .unwrap(),
            GH_FAILED
        );
    }
}
