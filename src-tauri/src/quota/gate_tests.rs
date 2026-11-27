//! The `quotaReads` gate (ADR-0014): a switched-off agent touches no sign-in and no network.
use super::*;
use crate::testing::TempDir;
use std::cell::Cell;

/// A present, well-formed CLI subscription sign-in with a synthetic token.
fn sign_in(dir: &TempDir, agent: AgentId) -> PathBuf {
    let (name, body) = match agent {
        AgentId::Claude => (
            ".credentials.json",
            r#"{"claudeAiOauth":{"accessToken":"synthetic-claude-token"}}"#,
        ),
        _ => (
            "auth.json",
            r#"{"auth_mode":"chatgpt","tokens":{"access_token":"synthetic-codex-token"}}"#,
        ),
    };
    let path = dir.0.join(name);
    std::fs::write(&path, body).unwrap();
    path
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
fn a_switched_off_read_resolves_no_path_reads_no_file_and_builds_no_request() {
    // Claude and Codex by default; OpenCode Go once its switch is turned off.
    let mut settings = AppSettings::default();
    settings.quota_reads.opencode = false;
    for agent in AgentId::ALL {
        let dir = TempDir::new("quota-gate-off");
        let path = sign_in(&dir, agent);
        let resolved = Cell::new(0);
        let fetched = Cell::new(0);
        let quota = read_with(
            agent,
            &settings,
            || {
                resolved.set(resolved.get() + 1);
                Ok(path.clone())
            },
            |_, _| {
                fetched.set(fetched.get() + 1);
                AgentQuota::unavailable("read")
            },
        );
        assert_eq!(quota, AgentQuota::off(), "{agent:?}");
        assert_eq!((resolved.get(), fetched.get()), (0, 0), "{agent:?}");
        // The production entry point with the real fetch: a request would have
        // ended in a provider detail, never in the empty `off` result.
        let quota = read(agent, None, &settings, || Ok(path.clone()));
        assert_eq!(quota, AgentQuota::off(), "{agent:?}");
    }
}

#[test]
fn a_switched_on_read_resolves_the_path_and_fetches_it() {
    let mut settings = AppSettings::default();
    settings.quota_reads.claude = true;
    let dir = TempDir::new("quota-gate-on");
    let path = sign_in(&dir, AgentId::Claude);
    let fetched = Cell::new(None);
    let quota = read_with(
        AgentId::Claude,
        &settings,
        || Ok(path.clone()),
        |agent, path| {
            fetched.set(Some((agent, path.to_path_buf())));
            AgentQuota::off()
        },
    );
    assert_eq!(quota, AgentQuota::off());
    assert_eq!(fetched.take(), Some((AgentId::Claude, path)));

    let fetched = Cell::new(false);
    let quota = read_with(
        AgentId::Claude,
        &settings,
        || Err("Der Anmeldeordner ist nicht verfügbar."),
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
