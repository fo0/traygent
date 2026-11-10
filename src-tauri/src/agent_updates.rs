//! Background updates of the locally installed agent CLIs (#16): each CLI's own update
//! command, run only while no process uses that CLI.
//!
//! Pure over [`Runner`] and [`Fs`]; the desktop shell owns the schedule thread and commits
//! every status to the service. The in-use check fails closed: a process list that cannot
//! be read counts every agent as in use, so an update never replaces a running CLI.
use crate::detect::{self, Candidate, Fs, Host, RunError, Runner};
use crate::models::{AgentId, AgentUpdateState, AgentUpdateStatus, PerAgent};
use chrono::{DateTime, Utc};
use std::time::Duration;

/// A background run starts this long after the previous one ended.
pub const INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);
/// The first background run waits this long after the app started.
pub const STARTUP_DELAY: Duration = Duration::from_secs(60);
/// How often the schedule thread looks whether a run is due.
pub const TICK: Duration = Duration::from_secs(60);
/// An update command downloads a release; the shell's update runner gives it this long.
pub const UPDATE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Windows PowerShell start plus one WMI query; the shell's process-list runner.
pub const LIST_TIMEOUT: Duration = Duration::from_secs(20);

/// Last line of the process list; without it the output was cut or the script failed.
const END_MARKER: &str = "traygent-end";

/// Lists the processes that may hold an agent CLI, already filtered and deduplicated so the
/// runner's output cap never hides one: `exe <image>` for a CLI binary, `pkg <match>` for a
/// node or bun process whose command line names an agent's npm package, then the end
/// marker. The Claude desktop app is `Claude.exe` too: a process from its install folders
/// (UNVERIFIED paths, conventional) is no CLI, one without a readable path still counts.
/// Windows PowerShell 5.1; no double quote anywhere (docs/adr/0003).
const LIST_SCRIPT: &str = r"$ErrorActionPreference = 'Stop'
$packages = 'claude-code|@opencode|opencode-ai|@openai[\\/]codex|codex\.js'
$desktop = '\\AnthropicClaude\\|\\WindowsApps\\Claude_'
$found = foreach ($process in Get-CimInstance Win32_Process -Property Name,ExecutablePath,CommandLine) {
  $name = ([string]$process.Name).ToLowerInvariant()
  if ($name -match '^(claude|opencode2?|codex)\.exe$') {
    if ([string]$process.ExecutablePath -notmatch $desktop) { 'exe ' + $name }
  }
  elseif ($name -match '^(node|bun)\.exe$') {
    foreach ($match in [regex]::Matches([string]$process.CommandLine, $packages, 'IgnoreCase')) {
      'pkg ' + $match.Value.ToLowerInvariant()
    }
  }
}
$found | Sort-Object -Unique
'traygent-end'";

/// The PowerShell argv of the process list; the script travels as `-EncodedCommand`, so
/// nothing of it reaches PowerShell's argument parsing (docs/adr/0003).
pub fn list_args() -> Vec<String> {
    let script = crate::launch::quote::encode_command(LIST_SCRIPT);
    let flags = ["-NoProfile", "-NonInteractive", "-EncodedCommand"];
    flags
        .into_iter()
        .map(String::from)
        .chain([script])
        .collect()
}

/// The CLI's own update command (docs/AGENT_RECIPES.md, `InstallInfo.update`).
pub fn update_args(agent: AgentId) -> &'static [&'static str] {
    match agent {
        AgentId::Claude | AgentId::Codex => &["update"],
        AgentId::Opencode => &["upgrade"],
    }
}

/// Whether the background schedule starts a run: never while the setting is off, the first
/// one [`STARTUP_DELAY`] after the app started, then [`INTERVAL`] after the last one ended.
pub fn due(enabled: bool, uptime: Duration, since_last_run: Option<Duration>) -> bool {
    enabled
        && match since_last_run {
            Some(elapsed) => elapsed >= INTERVAL,
            None => uptime >= STARTUP_DELAY,
        }
}

/// The agent one line of the process list belongs to; `None` for a line the script never
/// prints.
fn line_agent(line: &str) -> Option<AgentId> {
    let (kind, name) = line.split_once(' ')?;
    match (kind, name) {
        ("exe", "claude.exe") => Some(AgentId::Claude),
        ("exe", "opencode.exe" | "opencode2.exe") => Some(AgentId::Opencode),
        ("exe", "codex.exe") => Some(AgentId::Codex),
        ("pkg", "claude-code") => Some(AgentId::Claude),
        ("pkg", "@opencode" | "opencode-ai") => Some(AgentId::Opencode),
        ("pkg", "@openai\\codex" | "@openai/codex" | "codex.js") => Some(AgentId::Codex),
        _ => None,
    }
}

/// Which agents a running process uses. Fails closed — every agent in use — on a host
/// without the Windows process list, a runner error, a non-success exit, a missing end
/// marker or a line the script never prints.
pub fn in_use(host: &Host, runner: &dyn Runner) -> PerAgent<bool> {
    let all = || PerAgent::from_fn(|_| true);
    if !host.windows {
        return all();
    }
    let args = list_args();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = match runner.run(&host.powershell(), &args) {
        Ok(output) if output.success => output.stdout,
        _ => return all(),
    };
    let mut lines: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.pop() != Some(END_MARKER) {
        return all();
    }
    let mut used = PerAgent::<bool>::default();
    for line in lines {
        match line_agent(line) {
            Some(agent) => *used.get_mut(agent) = true,
            None => return all(),
        }
    }
    used
}

/// A status without versions, e.g. `running` before the agent's turn or `inUse`.
pub fn status(agent: AgentId, state: AgentUpdateState, now: DateTime<Utc>) -> AgentUpdateStatus {
    AgentUpdateStatus {
        agent,
        state,
        version: None,
        from: None,
        at: crate::recents::timestamp(now),
    }
}

/// The runners of one run: the process list, the `--version` probes and the update command
/// each get their own timeout from the shell.
pub struct Runners<'a> {
    pub list: &'a dyn Runner,
    pub probe: &'a dyn Runner,
    pub update: &'a dyn Runner,
}

fn version(cli: &str, runner: &dyn Runner) -> Result<Option<String>, RunError> {
    let output = runner.run(cli, &["--version"])?;
    Ok(detect::parse_version(&output.stdout))
}

/// Runs the agent's update command on its resolved CLI: `updated` when the version changed,
/// `current` when the command succeeded and it did not (or cannot be told), `failed` on a
/// non-success exit, a timeout or a CLI that does not start.
pub fn update_agent(
    agent: AgentId,
    cli: &Candidate,
    host: &Host,
    fs: &dyn Fs,
    runners: &Runners,
    clock: &dyn Fn() -> DateTime<Utc>,
) -> AgentUpdateStatus {
    let before = version(&cli.path, runners.probe).ok().flatten();
    let succeeded = runners
        .update
        .run(&cli.path, update_args(agent))
        .is_ok_and(|output| output.success);
    if !succeeded {
        return status(agent, AgentUpdateState::Failed, clock());
    }
    let after = match version(&cli.path, runners.probe) {
        // The update moved the CLI: resolve it again.
        Err(RunError::Missing) => {
            detect::probe_local(agent, host, fs, runners.probe)
                .0
                .version
        }
        found => found.ok().flatten(),
    };
    let updated = before.is_some() && after.is_some() && before != after;
    AgentUpdateStatus {
        version: after,
        from: before.filter(|_| updated),
        ..status(
            agent,
            if updated {
                AgentUpdateState::Updated
            } else {
                AgentUpdateState::Current
            },
            clock(),
        )
    }
}

/// One agent's turn in a run: `inUse` while a process uses its CLI (or the process list
/// cannot be read), else the update.
pub fn run_agent(
    agent: AgentId,
    cli: &Candidate,
    host: &Host,
    fs: &dyn Fs,
    runners: &Runners,
    clock: &dyn Fn() -> DateTime<Utc>,
) -> AgentUpdateStatus {
    if *in_use(host, runners.list).get(agent) {
        return status(agent, AgentUpdateState::InUse, clock());
    }
    update_agent(agent, cli, host, fs, runners, clock)
}

#[cfg(test)]
mod tests;
