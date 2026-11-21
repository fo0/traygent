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
/// runner's output cap never hides one: `exe <image> <path>` for a binary named like a CLI
/// (the path is missing when it cannot be read), `pkg <match>` for a node or bun process
/// whose command line names an agent's npm package, `pkg unknown` for one whose command line
/// cannot be read (another user's or an elevated process; it counts for every agent), then
/// the end marker. Which binary is a CLI is decided here in Rust ([`bundled`]).
/// Windows PowerShell 5.1; no double quote anywhere (docs/adr/0003).
const LIST_SCRIPT: &str = r"$ErrorActionPreference = 'Stop'
$packages = 'claude-code|@opencode|opencode-ai|@openai[\\/]codex|codex\.js'
$found = foreach ($process in Get-CimInstance Win32_Process -Property Name,ExecutablePath,CommandLine) {
  $name = ([string]$process.Name).ToLowerInvariant()
  if ($name -match '^(claude|opencode2?|codex)\.exe$') {
    ('exe ' + $name + ' ' + [string]$process.ExecutablePath).Trim()
  }
  elseif ($name -match '^(node|bun)\.exe$') {
    $line = [string]$process.CommandLine
    if ([string]::IsNullOrWhiteSpace($line)) { 'pkg unknown' }
    foreach ($match in [regex]::Matches($line, $packages, 'IgnoreCase')) {
      'pkg ' + $match.Value.ToLowerInvariant()
    }
  }
}
$found | Sort-Object -Unique
'traygent-end'";

/// Install folders of programs that ship their own binary under a CLI's image name and keep
/// it running in the background: the Claude desktop app (also `Claude.exe`) and the Codex
/// desktop app as Store packages or under `AnthropicClaude`, and the editor extensions of
/// both agents. The CLI's update command replaces none of them, so they do not hold the CLI
/// (#78). Lower case, matched anywhere in the path; UNVERIFIED paths, conventional.
const BUNDLED: [&str; 5] = [
    r"\anthropicclaude\",
    r"\windowsapps\claude_",
    r"\windowsapps\openai.",
    r"\extensions\anthropic.claude-code-",
    r"\extensions\openai.chatgpt-",
];

/// The folder of Store packages and of their app execution aliases.
const STORE: &str = r"\windowsapps\";

/// `by` of a skipped agent when the process list could not be read.
pub const LIST_UNREADABLE: &str = "Prozessliste nicht lesbar";
/// `by` of a skipped agent when only a node or bun process without a readable command line
/// holds it.
pub const PROCESS_UNREADABLE: &str = "node- oder bun-Prozess ohne lesbare Befehlszeile";

/// Whether a binary at this path belongs to another program's bundle; one without a
/// readable path never does, so it still counts as the CLI. Neither does the CLI the run
/// updates (`cli`, its resolved path): its own path is never a bundle, and when it is a
/// Store install (an app execution alias under `WindowsApps`) no Store package is one.
fn bundled(path: &str, cli: &str) -> bool {
    let (path, cli) = (path.to_lowercase(), cli.to_lowercase());
    let store = cli.contains(STORE);
    path != cli
        && BUNDLED
            .iter()
            .any(|folder| path.contains(folder) && !(store && folder.starts_with(STORE)))
}

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

/// What one line of the process list says.
#[derive(Debug, PartialEq, Eq)]
enum Line<'a> {
    /// A process holds this agent's CLI; the text names it for the status.
    Holds(AgentId, &'a str),
    /// A binary named like a CLI inside another program's bundle: no CLI.
    Bundled,
    /// `pkg unknown`: it may hold any agent.
    Unknown,
}

/// Reads one line of the process list for a run on the CLI at `cli`; `None` for a line the
/// script never prints.
fn read_line<'a>(line: &'a str, cli: &str) -> Option<Line<'a>> {
    let (kind, rest) = line.split_once(' ')?;
    match kind {
        "exe" => {
            let (name, path) = rest.split_once(' ').unwrap_or((rest, ""));
            let agent = match name {
                "claude.exe" => AgentId::Claude,
                "opencode.exe" | "opencode2.exe" => AgentId::Opencode,
                "codex.exe" => AgentId::Codex,
                _ => return None,
            };
            let path = path.trim();
            Some(match path {
                "" => Line::Holds(agent, name),
                path if bundled(path, cli) => Line::Bundled,
                path => Line::Holds(agent, path),
            })
        }
        "pkg" => match rest {
            "unknown" => Some(Line::Unknown),
            "claude-code" => Some(Line::Holds(AgentId::Claude, line)),
            "@opencode" | "opencode-ai" => Some(Line::Holds(AgentId::Opencode, line)),
            "@openai\\codex" | "@openai/codex" | "codex.js" => {
                Some(Line::Holds(AgentId::Codex, line))
            }
            _ => None,
        },
        _ => None,
    }
}

/// What holds each agent's CLI, read for a run on the CLI at `cli`: `None` when no process uses it, else the process for the
/// status — a path, an image name or a `pkg` line. Fails closed — every agent held, by
/// [`LIST_UNREADABLE`] — on a host without the Windows process list, a runner error, a
/// non-success exit, a missing end marker or a line the script never prints.
pub fn in_use(host: &Host, runner: &dyn Runner, cli: &str) -> PerAgent<Option<String>> {
    let all = || PerAgent::from_fn(|_| Some(LIST_UNREADABLE.to_string()));
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
    let mut used = PerAgent::<Option<String>>::default();
    let mut unknown = false;
    for line in lines {
        match read_line(line, cli) {
            Some(Line::Holds(agent, by)) => {
                used.get_mut(agent).get_or_insert_with(|| by.to_string());
            }
            Some(Line::Bundled) => {}
            Some(Line::Unknown) => unknown = true,
            None => return all(),
        }
    }
    if unknown {
        for agent in AgentId::ALL {
            used.get_mut(agent)
                .get_or_insert_with(|| PROCESS_UNREADABLE.to_string());
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
        by: None,
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
/// cannot be read), naming that process in `by`, else the update.
pub fn run_agent(
    agent: AgentId,
    cli: &Candidate,
    host: &Host,
    fs: &dyn Fs,
    runners: &Runners,
    clock: &dyn Fn() -> DateTime<Utc>,
) -> AgentUpdateStatus {
    if let Some(by) = in_use(host, runners.list, &cli.path).get(agent).clone() {
        return AgentUpdateStatus {
            by: Some(by),
            ..status(agent, AgentUpdateState::InUse, clock())
        };
    }
    update_agent(agent, cli, host, fs, runners, clock)
}

#[cfg(test)]
mod tests;
