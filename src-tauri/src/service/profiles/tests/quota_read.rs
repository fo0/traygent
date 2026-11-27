//! The hidden `/usage` read of Claude Code's quota (#110): the host's CLI in a folder that is
//! no project, the profile's directory as `quota_paths` chose it. Synthetic data only.
use super::*;
use crate::launch::tests::decode_command;
use crate::models::AgentUpdateState;

const USAGE: &str = "'-p' '/usage' '--output-format' 'json' '--no-session-persistence'";

#[test]
fn a_quota_read_runs_the_hosts_cli_in_the_app_data_dir() {
    let dir = TempDir::new("service-quota-read");
    let mut service = service(&dir);
    let app_dir = dir.0.to_str().unwrap().to_string();
    let host = windows_host();
    let in_app = fs().dir(&app_dir);
    let team = Some(r"C:\Users\max\.claude-team".to_string());
    let plan = service
        .prepare_quota_read(AgentId::Claude, team, &host, &in_app)
        .unwrap();
    assert_eq!(plan.cwd, app_dir);
    let script = decode_command(plan.args.last().unwrap());
    assert!(script.contains(&format!("; {SET_TEAM}; & '{CLAUDE_EXE}' {USAGE}; ")));
    // The default profile removes a stray variable while extra profiles exist.
    let plan = service
        .prepare_quota_read(AgentId::Claude, None, &host, &in_app)
        .unwrap();
    let script = decode_command(plan.args.last().unwrap());
    assert!(script.contains(&format!(
        "; Remove-Item Env:CLAUDE_CONFIG_DIR -ErrorAction SilentlyContinue; & '{CLAUDE_EXE}' {USAGE}; "
    )));
    // Without an app data dir the read starts in the user profile, as a sign-in does.
    let plan = service
        .prepare_quota_read(AgentId::Claude, None, &host, &fs())
        .unwrap();
    assert_eq!(plan.cwd, r"C:\Users\max");
    assert!(in_app.created().is_empty(), "a read creates nothing");

    let refused = |service: &Service, agent, fs: &FakeFs| {
        service
            .prepare_quota_read(agent, None, &host, fs)
            .unwrap_err()
    };
    assert_eq!(
        refused(&service, AgentId::Claude, &FakeFs::default()),
        "Claude Code ist nicht installiert."
    );
    assert_eq!(
        refused(&service, AgentId::Codex, &in_app),
        "Codex liest sein Kontingent nicht über die CLI."
    );
    let now = "2026-11-27T08:00:00Z".parse().unwrap();
    let running = crate::agent_updates::status(AgentId::Claude, AgentUpdateState::Running, now);
    service.set_agent_update(running);
    assert_eq!(
        refused(&service, AgentId::Claude, &in_app),
        "Claude Code wird gerade aktualisiert."
    );
}
