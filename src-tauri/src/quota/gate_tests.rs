//! The `quotaReads` gate (ADR-0014): a switched-off agent touches no sign-in, no network and
//! starts no process.
use super::claude_cli::CliEnd;
use super::*;
use crate::launch::HiddenLogin;
use crate::testing::TempDir;
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Counts the CLI runs it was asked for; answers each with an empty, failed run.
#[derive(Default)]
struct Counting(AtomicUsize);

impl CliRunner for Counting {
    fn run(&self, _: &HiddenLogin, _: Duration) -> CliEnd {
        self.0.fetch_add(1, Ordering::SeqCst);
        CliEnd::NotStarted
    }
}

fn cli_read() -> CliRead {
    CliRead::new(HiddenLogin {
        program: r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".into(),
        args: Vec::new(),
        cwd: r"C:\Users\max".into(),
    })
}

/// A present, well-formed sign-in with a synthetic key for a file read, the CLI for Claude.
fn source(dir: &TempDir, agent: AgentId) -> Source {
    if agent == AgentId::Claude {
        return Source::Cli(cli_read());
    }
    let path = dir.0.join("auth.json");
    let body = r#"{"auth_mode":"chatgpt","tokens":{"access_token":"synthetic-codex-token"}}"#;
    std::fs::write(&path, body).unwrap();
    Source::File(path)
}

#[test]
fn the_defaults_read_only_opencode_go() {
    let settings = AppSettings::default();
    assert!(!settings.quota_reads.claude);
    assert!(!settings.quota_reads.codex);
    assert!(settings.quota_reads.opencode);
}

#[test]
fn a_partial_object_over_ipc_keeps_the_other_agents_defaults() {
    let settings: AppSettings = serde_json::from_str(r#"{"quotaReads":{"claude":true}}"#).unwrap();
    assert!(settings.quota_reads.claude);
    assert!(!settings.quota_reads.codex);
    assert!(settings.quota_reads.opencode);
}

#[test]
fn a_switched_off_read_resolves_nothing_reads_no_file_and_starts_no_process() {
    // Claude and Codex by default; OpenCode Go once its switch is turned off.
    let mut settings = AppSettings::default();
    settings.quota_reads.opencode = false;
    for agent in AgentId::ALL {
        let dir = TempDir::new("quota-gate-off");
        let resolved = Cell::new(0);
        let runner = Counting::default();
        let quota = read_with(
            agent,
            &settings,
            || {
                resolved.set(resolved.get() + 1);
                Ok(source(&dir, agent))
            },
            |agent, source| fetch(agent, "default", source, &runner),
        );
        assert_eq!(quota, AgentQuota::off(), "{agent:?}");
        assert_eq!(resolved.get(), 0, "{agent:?}");
        assert_eq!(runner.0.load(Ordering::SeqCst), 0, "{agent:?}");
        // The production entry point with the real fetch and the real process runner: a
        // read would have ended in a detail, never in the empty `off` result.
        let quota = read(agent, None, &settings, || Ok(source(&dir, agent)));
        assert_eq!(quota, AgentQuota::off(), "{agent:?}");
    }
}

#[test]
fn a_switched_on_claude_read_runs_the_cli_once_and_never_a_file() {
    let mut settings = AppSettings::default();
    settings.quota_reads.claude = true;
    let runner = Counting::default();
    let quota = read_with(
        AgentId::Claude,
        &settings,
        || Ok(Source::Cli(cli_read())),
        |agent, source| fetch(agent, "default", source, &runner),
    );
    assert_eq!(runner.0.load(Ordering::SeqCst), 1);
    assert!(quota.windows.is_empty() && quota.detail.is_some());
    // A file source for Claude Code is never read over HTTP (ADR-0015).
    let dir = TempDir::new("quota-gate-claude-file");
    let file = dir.0.join(".credentials.json");
    let quota = fetch(AgentId::Claude, "default", Source::File(file), &runner);
    assert_eq!(quota, AgentQuota::unavailable(claude_usage::UNREADABLE));
    assert_eq!(runner.0.load(Ordering::SeqCst), 1);
}

#[test]
fn a_switched_on_read_resolves_the_source_and_fetches_it() {
    let mut settings = AppSettings::default();
    settings.quota_reads.codex = true;
    let dir = TempDir::new("quota-gate-on");
    let fetched = Cell::new(None);
    let quota = read_with(
        AgentId::Codex,
        &settings,
        || Ok(source(&dir, AgentId::Codex)),
        |agent, source| {
            let Source::File(path) = source else {
                panic!("a file source")
            };
            fetched.set(Some((agent, path)));
            AgentQuota::off()
        },
    );
    assert_eq!(quota, AgentQuota::off());
    assert_eq!(
        fetched.take(),
        Some((AgentId::Codex, dir.0.join("auth.json")))
    );

    let fetched = Cell::new(false);
    let quota = read_with(
        AgentId::Codex,
        &settings,
        || Err("Der Anmeldeordner ist nicht verfügbar.".into()),
        |_, _| {
            fetched.set(true);
            AgentQuota::off()
        },
    );
    assert_eq!(
        quota,
        AgentQuota::unavailable("Der Anmeldeordner ist nicht verfügbar.")
    );
    assert!(!fetched.get());
}
