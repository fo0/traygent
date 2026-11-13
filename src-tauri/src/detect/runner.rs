//! Runs a probe process: closed stdin, capped stdout, stderr ignored, a hard timeout and no
//! console window. Tests inject a fake [`Runner`].
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Probe output beyond this is drained and dropped.
pub const MAX_OUTPUT: usize = 64 * 1024;
pub const TIMEOUT: Duration = Duration::from_secs(5);
/// How long a timed-out run waits for `taskkill` to end its process tree.
#[cfg(windows)]
const KILL_TREE_WAIT: Duration = Duration::from_secs(5);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunOutput {
    pub success: bool,
    pub stdout: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunError {
    /// The program does not exist (or vanished since it was resolved).
    Missing,
    Timeout,
    Failed,
}

pub trait Runner: Sync {
    fn run(&self, program: &str, args: &[&str]) -> Result<RunOutput, RunError>;
}

pub struct SystemRunner {
    pub timeout: Duration,
    /// On a timeout, end the child's whole process tree first (Windows): an npm `.cmd` shim
    /// would otherwise leave its `node` grandchild running.
    pub kill_tree: bool,
    /// Extra environment variables of the child: values that must not appear in its command
    /// line (docs/adr/0003).
    pub env: Vec<(String, String)>,
}

impl Default for SystemRunner {
    fn default() -> Self {
        Self {
            timeout: TIMEOUT,
            kill_tree: false,
            env: Vec::new(),
        }
    }
}

impl Runner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<RunOutput, RunError> {
        let mut command = Command::new(program);
        command
            .args(args)
            .envs(self.env.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => RunError::Missing,
            _ => RunError::Failed,
        })?;
        let stdout = child.stdout.take();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(read_capped(stdout, MAX_OUTPUT));
        });
        let deadline = Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                result => {
                    #[cfg(windows)]
                    if self.kill_tree {
                        kill_tree(child.id());
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(if result.is_ok() {
                        RunError::Timeout
                    } else {
                        RunError::Failed
                    });
                }
            }
        };
        // A grandchild (npm shim → node) may keep the pipe open after the shim exited.
        let bytes = receiver
            .recv_timeout(Duration::from_millis(500))
            .unwrap_or_default();
        Ok(RunOutput {
            success: status.success(),
            stdout: String::from_utf8_lossy(&bytes).into_owned(),
        })
    }
}

/// Ends the process `pid` and its descendants: `taskkill /T /F` by absolute path, no window,
/// waited for at most [`KILL_TREE_WAIT`]. Best effort; the caller still kills the child.
#[cfg(windows)]
fn kill_tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    let host = super::Host {
        windows: true,
        system_root: std::env::var("SystemRoot").ok(),
        ..super::Host::default()
    };
    let pid = pid.to_string();
    let spawned = Command::new(host.taskkill())
        .args(["/T", "/F", "/PID", pid.as_str()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    let Ok(mut taskkill) = spawned else {
        return;
    };
    let deadline = Instant::now() + KILL_TREE_WAIT;
    while matches!(taskkill.try_wait(), Ok(None)) {
        if Instant::now() >= deadline {
            let _ = taskkill.kill();
            let _ = taskkill.wait();
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Keeps the first `cap` bytes and drains the rest so the child never blocks on a full pipe.
pub fn read_capped(reader: Option<impl Read>, cap: usize) -> Vec<u8> {
    let Some(mut reader) = reader else {
        return Vec::new();
    };
    let mut kept = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                let room = cap.saturating_sub(kept.len());
                kept.extend_from_slice(&buffer[..read.min(room)]);
            }
        }
    }
    kept
}
