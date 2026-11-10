use super::*;
use crate::launch::tests::decode_command;
use crate::testing::{windows_host, FakeFs};
use std::collections::HashMap;
use std::sync::Mutex;

const POWERSHELL: &str = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
const CLAUDE: &str = r"C:\Users\max\.local\bin\claude.exe";
const CODEX: &str = r"C:\Users\max\AppData\Roaming\npm\codex.cmd";

/// Replies per `<program> <args…>`, consumed in order (the last one repeats); anything
/// else fails like a crashing process.
#[derive(Default)]
struct FakeRunner {
    replies: Mutex<HashMap<String, Vec<Result<RunOutput, RunError>>>>,
    calls: Mutex<Vec<String>>,
}

use crate::detect::RunOutput;

impl FakeRunner {
    fn result(self, call: &str, result: Result<RunOutput, RunError>) -> Self {
        let mut replies = self.replies.lock().unwrap();
        replies.entry(call.into()).or_default().push(result);
        drop(replies);
        self
    }
    fn reply(self, call: &str, stdout: &str) -> Self {
        let output = RunOutput {
            success: true,
            stdout: stdout.into(),
        };
        self.result(call, Ok(output))
    }
    fn exit_failure(self, call: &str) -> Self {
        self.result(call, Ok(RunOutput::default()))
    }
    /// The process list answers with these lines.
    fn processes(self, stdout: &str) -> Self {
        self.reply(&list_call(), stdout)
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Runner for FakeRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<RunOutput, RunError> {
        let call = format!("{program} {}", args.join(" "));
        self.calls.lock().unwrap().push(call.clone());
        let mut replies = self.replies.lock().unwrap();
        match replies.get_mut(&call) {
            Some(list) if list.len() > 1 => list.remove(0),
            Some(list) => list[0].clone(),
            None => Err(RunError::Failed),
        }
    }
}

fn list_call() -> String {
    format!("{POWERSHELL} {}", list_args().join(" "))
}

fn cli(path: &str) -> Candidate {
    Candidate {
        path: path.into(),
        shim: path.ends_with(".cmd"),
    }
}

fn clock() -> DateTime<Utc> {
    "2026-11-10T08:00:00Z".parse().unwrap()
}

fn used(stdout: &str) -> [bool; 3] {
    let runner = FakeRunner::default().processes(stdout);
    let used = in_use(&windows_host(), &runner);
    AgentId::ALL.map(|agent| *used.get(agent))
}

/// `update_agent` / `run_agent` with one runner in every role.
fn run(agent: AgentId, path: &str, runner: &FakeRunner) -> AgentUpdateStatus {
    let runners = Runners {
        list: runner,
        probe: runner,
        update: runner,
    };
    let fs = FakeFs::with(&[path]);
    run_agent(agent, &cli(path), &windows_host(), &fs, &runners, &clock)
}

#[test]
fn the_process_list_is_an_encoded_powershell_script_without_double_quotes() {
    let args = list_args();
    assert_eq!(
        args[..3],
        ["-NoProfile", "-NonInteractive", "-EncodedCommand"]
    );
    let script = decode_command(&args[3]);
    assert_eq!(script, LIST_SCRIPT);
    assert!(!script.contains('"'));
    assert!(script.contains("Get-CimInstance Win32_Process"));
    assert!(script.trim_end().ends_with(&format!("'{END_MARKER}'")));
}

#[test]
fn update_commands_are_the_catalog_ones() {
    for entry in crate::catalog::catalog() {
        let command = entry.install.update.expect("an update command");
        let args = update_args(entry.id);
        assert_eq!(args.len(), 1);
        assert!(command.ends_with(&format!(" {}", args[0])), "{command}");
    }
}

#[test]
fn a_running_cli_binary_marks_only_its_agent() {
    assert_eq!(used("traygent-end\n"), [false, false, false]);
    assert_eq!(
        used("exe claude.exe\r\ntraygent-end\r\n"),
        [true, false, false]
    );
    assert_eq!(
        used("exe opencode.exe\ntraygent-end\n"),
        [false, true, false]
    );
    assert_eq!(
        used("exe opencode2.exe\ntraygent-end\n"),
        [false, true, false]
    );
    assert_eq!(used("exe codex.exe\ntraygent-end\n"), [false, false, true]);
}

#[test]
fn a_node_shim_naming_the_package_marks_only_its_agent() {
    assert_eq!(
        used("pkg claude-code\ntraygent-end\n"),
        [true, false, false]
    );
    assert_eq!(used("pkg @opencode\ntraygent-end\n"), [false, true, false]);
    assert_eq!(
        used("pkg opencode-ai\ntraygent-end\n"),
        [false, true, false]
    );
    assert_eq!(
        used("pkg @openai\\codex\ntraygent-end\n"),
        [false, false, true]
    );
    assert_eq!(
        used("pkg @openai/codex\ntraygent-end\n"),
        [false, false, true]
    );
    assert_eq!(used("pkg codex.js\ntraygent-end\n"), [false, false, true]);
    assert_eq!(
        used("exe claude.exe\npkg codex.js\n\ntraygent-end\n"),
        [true, false, true]
    );
}

#[test]
fn an_unreadable_process_list_counts_every_agent_as_in_use() {
    let all = [true, true, true];
    // No end marker: cut output or a script that stopped.
    assert_eq!(used(""), all);
    assert_eq!(used("exe claude.exe\n"), all);
    assert_eq!(used("traygent-end\nexe claude.exe\n"), all);
    // A line the script never prints.
    assert_eq!(used("Get-CimInstance : Access denied\ntraygent-end\n"), all);
    let host = windows_host();
    let flags = |runner: &FakeRunner| AgentId::ALL.map(|agent| *in_use(&host, runner).get(agent));
    assert_eq!(flags(&FakeRunner::default()), all);
    for error in [RunError::Missing, RunError::Timeout, RunError::Failed] {
        let runner = FakeRunner::default().result(&list_call(), Err(error));
        assert_eq!(flags(&runner), all, "{error:?}");
    }
    let failed = RunOutput {
        success: false,
        stdout: "traygent-end\n".into(),
    };
    let runner = FakeRunner::default().result(&list_call(), Ok(failed));
    assert_eq!(flags(&runner), all);
    // No Windows process list on another host: unknown, so in use; nothing is run.
    let runner = FakeRunner::default().processes("traygent-end\n");
    let other = Host::default();
    assert_eq!(
        AgentId::ALL.map(|agent| *in_use(&other, &runner).get(agent)),
        all
    );
    assert!(runner.calls().is_empty());
}

#[test]
fn a_node_process_with_an_unreadable_command_line_counts_for_every_agent() {
    assert!(LIST_SCRIPT.contains("IsNullOrWhiteSpace($line)) { 'pkg unknown' }"));
    assert_eq!(line_agent("pkg unknown"), None);
    assert_eq!(used("pkg unknown\ntraygent-end\n"), [true, true, true]);
    assert_eq!(
        used("exe claude.exe\npkg unknown\ntraygent-end\n"),
        [true, true, true]
    );
}

#[test]
fn an_agent_in_use_is_skipped_without_running_its_update() {
    let runner = FakeRunner::default()
        .processes("exe claude.exe\ntraygent-end\n")
        .reply(&format!("{CLAUDE} --version"), "2.1.317 (Claude Code)")
        .reply(&format!("{CLAUDE} update"), "");
    let status = run(AgentId::Claude, CLAUDE, &runner);
    assert_eq!(
        status,
        super::status(AgentId::Claude, AgentUpdateState::InUse, clock())
    );
    assert_eq!(runner.calls(), [list_call()]);
}

#[test]
fn a_running_claude_does_not_block_codex() {
    let runner = FakeRunner::default()
        .processes("exe claude.exe\npkg claude-code\ntraygent-end\n")
        .reply(&format!("{CODEX} --version"), "codex-cli 0.166.0")
        .reply(&format!("{CODEX} --version"), "codex-cli 0.167.0")
        .reply(&format!("{CODEX} update"), "updated");
    let status = run(AgentId::Codex, CODEX, &runner);
    assert_eq!(status.state, AgentUpdateState::Updated);
    assert_eq!(status.from.as_deref(), Some("0.166.0"));
    assert_eq!(status.version.as_deref(), Some("0.167.0"));
    assert_eq!(status.at, "2026-11-10T08:00:00Z");
    assert_eq!(
        runner.calls()[1..],
        [
            format!("{CODEX} --version"),
            format!("{CODEX} update"),
            format!("{CODEX} --version")
        ]
    );
}

#[test]
fn an_unchanged_version_is_current() {
    let runner = FakeRunner::default()
        .processes("traygent-end\n")
        .reply(&format!("{CLAUDE} --version"), "2.1.317 (Claude Code)")
        .reply(&format!("{CLAUDE} update"), "up to date");
    let status = run(AgentId::Claude, CLAUDE, &runner);
    assert_eq!(status.state, AgentUpdateState::Current);
    assert_eq!(status.version.as_deref(), Some("2.1.317"));
    assert_eq!(status.from, None);
    // A version that cannot be read on either side is not an update.
    let runner = FakeRunner::default()
        .processes("traygent-end\n")
        .reply(&format!("{CLAUDE} --version"), "?")
        .reply(&format!("{CLAUDE} --version"), "2.1.317")
        .reply(&format!("{CLAUDE} update"), "");
    let status = run(AgentId::Claude, CLAUDE, &runner);
    assert_eq!(status.state, AgentUpdateState::Current);
    assert_eq!(
        (status.version.as_deref(), status.from),
        (Some("2.1.317"), None)
    );
}

#[test]
fn a_failing_update_command_is_failed() {
    let version = format!("{CLAUDE} --version");
    let update = format!("{CLAUDE} update");
    let base = || {
        FakeRunner::default()
            .processes("traygent-end\n")
            .reply(&version, "2.1.317")
    };
    let cases = [
        base().exit_failure(&update),
        base().result(&update, Err(RunError::Timeout)),
        base().result(&update, Err(RunError::Missing)),
        base(),
    ];
    for runner in cases {
        let status = run(AgentId::Claude, CLAUDE, &runner);
        assert_eq!(
            status,
            super::status(AgentId::Claude, AgentUpdateState::Failed, clock())
        );
    }
}

#[test]
fn opencode_upgrades_and_a_moved_cli_is_resolved_again() {
    let old = r"C:\Users\max\AppData\Roaming\npm\opencode2.cmd";
    let new = r"C:\Users\max\.opencode\bin\opencode.exe";
    let runner = FakeRunner::default()
        .processes("traygent-end\n")
        .reply(&format!("{old} --version"), "opencode v2.2.5")
        .result(&format!("{old} --version"), Err(RunError::Missing))
        .reply(&format!("{old} upgrade"), "")
        .reply(&format!("{new} --version"), "opencode v2.3.0");
    let runners = Runners {
        list: &runner,
        probe: &runner,
        update: &runner,
    };
    let fs = FakeFs::with(&[new]);
    let host = windows_host();
    let status = run_agent(AgentId::Opencode, &cli(old), &host, &fs, &runners, &clock);
    assert_eq!(status.state, AgentUpdateState::Updated);
    assert_eq!(status.from.as_deref(), Some("2.2.5"));
    assert_eq!(status.version.as_deref(), Some("2.3.0"));
}

#[test]
fn the_schedule_waits_for_the_setting_the_start_and_the_interval() {
    let minute = Duration::from_secs(60);
    let hour = 60 * minute;
    // Off: never, whatever the clock says.
    assert!(!due(false, 10 * hour, None));
    assert!(!due(false, 10 * hour, Some(5 * hour)));
    // First run: one startup delay after the app started.
    assert!(!due(true, Duration::ZERO, None));
    assert!(!due(true, STARTUP_DELAY - Duration::from_secs(1), None));
    assert!(due(true, STARTUP_DELAY, None));
    // Then every four hours after the last run ended.
    assert!(!due(true, 5 * hour, Some(Duration::ZERO)));
    assert!(!due(true, 5 * hour, Some(4 * hour - minute)));
    assert!(due(true, 5 * hour, Some(4 * hour)));
    assert!(due(true, 9 * hour, Some(5 * hour)));
}
