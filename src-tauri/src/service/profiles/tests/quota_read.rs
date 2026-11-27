//! The hidden `/usage` read of Claude Code's quota (#110): the host's CLI in a folder that is
//! no project, the profile's directory as `quota_paths` chose it. Synthetic data only.
use super::*;
use crate::launch::tests::decode_command;
use crate::models::AgentUpdateState;

const USAGE: &str = "'-p' '/usage' '--output-format' 'json' '--no-session-persistence' \
                     '--setting-sources' 'user'";
const TEAM_DIR: &str = r"C:\Users\max\.claude-team";

fn team() -> Option<String> {
    Some(TEAM_DIR.to_string())
}

#[test]
fn a_quota_read_runs_the_hosts_cli_in_the_app_data_dir() {
    let dir = TempDir::new("service-quota-read");
    let mut service = service(&dir);
    let app_dir = dir.0.to_str().unwrap().to_string();
    let host = windows_host();
    let in_app = fs().dir(&app_dir).dir(TEAM_DIR);
    let plan = service
        .prepare_quota_read(AgentId::Claude, team(), &host, &in_app)
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
    assert!(in_app.created().is_empty(), "a read creates nothing");

    let refused = |service: &Service, agent, fs: &FakeFs| {
        service
            .prepare_quota_read(agent, None, &host, fs)
            .unwrap_err()
    };
    // Without an app data dir no folder is safe to start in: the user profile or `C:\` could
    // be a project whose settings and hooks would load.
    assert_eq!(
        refused(&service, AgentId::Claude, &fs()),
        "Das Kontingent wird nicht gelesen: kein App-Datenordner verfügbar."
    );
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

/// The CLI writes `.claude.json` into the profile's directory: into someone else's folder it
/// would make that folder look like Claude Code's, and the next sync would adopt it.
#[test]
fn a_quota_read_never_writes_into_a_foreign_or_missing_profile_dir() {
    let dir = TempDir::new("service-quota-read-foreign");
    let service = service(&dir);
    let app_dir = dir.0.to_str().unwrap().to_string();
    let host = windows_host();
    let mut foreign = fs().dir(&app_dir).dir(TEAM_DIR);
    foreign.foreign.insert(TEAM_DIR.into());
    assert_eq!(
        service
            .prepare_quota_read(AgentId::Claude, team(), &host, &foreign)
            .unwrap_err(),
        "Der Profilordner gehört nicht zu Claude Code."
    );
    // A profile never signed in has no directory yet: nothing starts, nothing is created.
    let missing = fs().dir(&app_dir);
    assert_eq!(
        service
            .prepare_quota_read(AgentId::Claude, team(), &host, &missing)
            .unwrap_err(),
        "Dieses Profil ist noch nicht angemeldet."
    );
    assert!(foreign.created().is_empty() && missing.created().is_empty());
}
