use super::*;

#[test]
fn lists_the_three_agents_in_contract_order() {
    let ids: Vec<_> = catalog().into_iter().map(|entry| entry.id).collect();
    assert_eq!(ids, AgentId::ALL);
    let labels: Vec<_> = catalog().into_iter().map(|entry| entry.label).collect();
    assert_eq!(labels, ["Claude Code", "OpenCode", "Codex"]);
}

#[test]
fn every_entry_starts_with_default_and_supports_new_sessions() {
    for entry in catalog() {
        assert_eq!(entry.models[0].id, "default", "{:?}", entry.id);
        assert_eq!(entry.efforts[0].id, "default", "{:?}", entry.id);
        assert!(entry.sessions.contains(&SessionMode::New));
        assert_eq!(entry.environments, [EnvId::Local, EnvId::Docker]);
        assert!(entry.free_model);
        let efforts: Vec<_> = entry.efforts.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            efforts,
            ["default", "low", "medium", "high", "xhigh", "max"]
        );
    }
}

#[test]
fn permissions_are_ask_auto_bypass_with_german_labels() {
    for entry in catalog() {
        let ids: Vec<_> = entry.permissions.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            [PermissionId::Ask, PermissionId::Auto, PermissionId::Bypass]
        );
        let labels: Vec<_> = entry.permissions.iter().map(|p| p.label.as_str()).collect();
        // The bypass choice is named the way the CLI names it.
        let bypass = match entry.id {
            AgentId::Claude => "Bypass",
            AgentId::Opencode | AgentId::Codex => "YOLO",
        };
        assert_eq!(labels, ["Nachfragen", "Auto", bypass]);
        let danger: Vec<_> = entry.permissions.iter().map(|p| p.danger).collect();
        assert_eq!(danger, [false, false, true]);
        assert!(entry.permissions.iter().all(|p| !p.hint.is_empty()));
    }
}

#[test]
fn models_and_sessions_follow_the_recipes() {
    let claude: Vec<_> = entry(AgentId::Claude)
        .models
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(
        claude,
        ["default", "fable", "opus", "sonnet", "haiku", "opusplan"]
    );
    assert_eq!(entry(AgentId::Opencode).models.len(), 1);
    let codex = entry(AgentId::Codex).models;
    assert_eq!(codex.len(), 10);
    assert_eq!(codex[1].id, "gpt-6.1-sol");
    assert_eq!(codex[9].id, "gpt-5.5");
    assert_eq!(
        entry(AgentId::Opencode).sessions,
        [SessionMode::New, SessionMode::Continue]
    );
    let full = [SessionMode::New, SessionMode::Continue, SessionMode::Resume];
    assert_eq!(entry(AgentId::Claude).sessions, full);
    assert_eq!(entry(AgentId::Codex).sessions, full);
}

#[test]
fn install_commands_are_the_official_ones() {
    let claude = entry(AgentId::Claude).install;
    assert_eq!(claude.command, "irm https://claude.ai/install.ps1 | iex");
    assert_eq!(claude.update.as_deref(), Some("claude update"));
    let opencode = entry(AgentId::Opencode).install;
    assert_eq!(opencode.command, "npm install -g @opencode/cli@latest");
    assert_eq!(opencode.update.as_deref(), Some("opencode upgrade"));
    let codex = entry(AgentId::Codex).install;
    assert_eq!(
        codex.command,
        "irm https://chatgpt.com/codex/install.ps1 | iex"
    );
    assert_eq!(codex.update.as_deref(), Some("codex update"));
    for entry in catalog() {
        assert!(entry.install.docs_url.starts_with("https://"));
        // Installs run inside a PowerShell -Command script passed through wt.exe.
        assert!(!entry.install.command.contains(['"', ';']));
    }
}
