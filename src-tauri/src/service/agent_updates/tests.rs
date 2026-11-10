use super::*;
use crate::detect::Detection;
use crate::models::{AppSettings, EnvId, LaunchOptions, LaunchRequest, PerAgent, SessionMode};
use crate::service::initial_statuses;
use crate::testing::{windows_host, FakeFs, TempDir};
use std::time::Duration;

const CLAUDE_EXE: &str = r"C:\Users\max\.local\bin\claude.exe";
const CODEX_CMD: &str = r"C:\Users\max\AppData\Roaming\npm\codex.cmd";
const DOCKER: &str = r"C:\Program Files\Docker\Docker\resources\bin\docker.exe";
const FOLDER: &str = r"C:\Code\demo";

fn status(agent: AgentId, state: AgentUpdateState) -> AgentUpdateStatus {
    let now = "2026-11-10T08:00:00Z".parse().unwrap();
    agent_updates::status(agent, state, now)
}

/// A finished detection with Claude (exe) and Codex (npm shim) ready, OpenCode missing.
fn detected(service: &mut Service) {
    let clis = PerAgent {
        claude: Some(Candidate {
            path: CLAUDE_EXE.into(),
            shim: false,
        }),
        opencode: None,
        codex: Some(Candidate {
            path: CODEX_CMD.into(),
            shim: true,
        }),
    };
    let (agents, environments) = initial_statuses();
    assert!(service.begin_detection());
    service.finish_detection(Some(Detection {
        agents,
        environments,
        clis,
        docker: Some(DOCKER.into()),
        checked_at: "2026-11-10T07:59:00Z".into(),
    }));
}

fn request(agent: AgentId, env: EnvId) -> LaunchRequest {
    LaunchRequest {
        agent,
        env,
        folder: FOLDER.into(),
        repo: None,
        options: LaunchOptions::default(),
        session: SessionMode::New,
        ports: Vec::new(),
    }
}

#[test]
fn a_second_run_does_not_begin_while_one_is_in_progress() {
    let mut service = Service::read_only("nur lesen");
    detected(&mut service);
    let revision = service.snapshot().revision;
    assert!(service.begin_agent_updates());
    assert!(service.snapshot().revision > revision);
    assert!(!service.begin_agent_updates());
    assert_eq!(service.start_agent_updates(), Err(UPDATE_RUNNING.into()));
    assert_eq!(UPDATE_RUNNING, "Agenten werden bereits aktualisiert.");
    service.finish_agent_updates(Instant::now());
    assert!(service.begin_agent_updates());
}

#[test]
fn a_run_targets_the_ready_local_clis_of_a_finished_detection() {
    let mut service = Service::read_only("nur lesen");
    // Before the first detection and while one runs there is nothing to update yet.
    assert_eq!(service.agent_update_targets(), None);
    assert_eq!(service.start_agent_updates(), Err(UPDATE_DETECTING.into()));
    assert!(
        service.begin_agent_updates(),
        "the refused start began no run"
    );
    service.finish_agent_updates(Instant::now());
    detected(&mut service);
    let targets = service.start_agent_updates().unwrap();
    let found: Vec<_> = targets
        .iter()
        .map(|(agent, cli)| (*agent, cli.path.as_str(), cli.shim))
        .collect();
    assert_eq!(
        found,
        [
            (AgentId::Claude, CLAUDE_EXE, false),
            (AgentId::Codex, CODEX_CMD, true)
        ]
    );
    service.finish_agent_updates(Instant::now());
    assert!(service.begin_detection());
    assert_eq!(service.agent_update_targets(), None);
}

#[test]
fn statuses_reach_the_snapshot_in_catalog_order_and_bump_the_revision() {
    let mut service = Service::read_only("nur lesen");
    assert!(service.snapshot().agent_updates.is_empty());
    assert!(service.begin_agent_updates());
    let revision = service.snapshot().revision;
    service.set_agent_update(status(AgentId::Codex, AgentUpdateState::Running));
    assert_eq!(service.snapshot().revision, revision + 1);
    service.set_agent_update(status(AgentId::Claude, AgentUpdateState::InUse));
    let listed = |service: &Service| -> Vec<_> {
        let statuses = service.snapshot().agent_updates;
        statuses.iter().map(|s| (s.agent, s.state)).collect()
    };
    assert_eq!(
        listed(&service),
        [
            (AgentId::Claude, AgentUpdateState::InUse),
            (AgentId::Codex, AgentUpdateState::Running)
        ]
    );
    assert!(service.agent_updating(AgentId::Codex));
    assert!(!service.agent_updating(AgentId::Claude));
    assert!(!service.agent_updating(AgentId::Opencode));
    service.set_agent_update(status(AgentId::Codex, AgentUpdateState::Current));
    assert!(!service.agent_updating(AgentId::Codex));
    assert_eq!(listed(&service)[1].1, AgentUpdateState::Current);
}

#[test]
fn a_run_that_died_leaves_no_agent_running() {
    let mut service = Service::read_only("nur lesen");
    assert!(service.begin_agent_updates());
    service.set_agent_update(status(AgentId::Claude, AgentUpdateState::Updated));
    service.set_agent_update(status(AgentId::Codex, AgentUpdateState::Running));
    let revision = service.snapshot().revision;
    service.finish_agent_updates(Instant::now());
    let snapshot = service.snapshot();
    assert!(snapshot.revision > revision);
    let states: Vec<_> = snapshot.agent_updates.iter().map(|s| s.state).collect();
    assert_eq!(
        states,
        [AgentUpdateState::Updated, AgentUpdateState::Failed]
    );
    assert!(!service.agent_updating(AgentId::Codex));
    assert!(service.begin_agent_updates());
}

#[test]
fn a_local_start_is_refused_only_while_its_agent_updates() {
    let mut service = Service::read_only("nur lesen");
    detected(&mut service);
    let host = windows_host();
    let fs = FakeFs::with(&[CLAUDE_EXE, CODEX_CMD, DOCKER]).dir(FOLDER);
    let launch = |service: &Service, agent, env| {
        service
            .prepare_launch(&request(agent, env), &host, &fs, "abc")
            .map(|_| ())
    };
    assert!(service.begin_agent_updates());
    service.set_agent_update(status(AgentId::Claude, AgentUpdateState::Running));
    assert_eq!(
        launch(&service, AgentId::Claude, EnvId::Local),
        Err("Claude Code wird gerade aktualisiert.".into())
    );
    // Docker runs its own CLI from a volume; another agent's local CLI is untouched.
    assert_eq!(launch(&service, AgentId::Claude, EnvId::Docker), Ok(()));
    assert_eq!(launch(&service, AgentId::Codex, EnvId::Local), Ok(()));
    service.set_agent_update(status(AgentId::Claude, AgentUpdateState::Updated));
    assert_eq!(launch(&service, AgentId::Claude, EnvId::Local), Ok(()));
    service.set_agent_update(status(AgentId::Codex, AgentUpdateState::InUse));
    assert_eq!(launch(&service, AgentId::Codex, EnvId::Local), Ok(()));
}

#[test]
fn the_schedule_follows_the_setting_the_start_and_the_last_run() {
    let dir = TempDir::new("service-agent-updates");
    // A state file from before the setting loads with it off.
    std::fs::write(
        dir.0.join("state.json"),
        r#"{"version":1,"settings":{"terminal":"console"}}"#,
    )
    .unwrap();
    let mut service = Service::open(&dir.0);
    assert!(!service.settings().agent_updates);
    let started = Instant::now();
    let after = |duration: Duration| started + duration;
    let late = after(agent_updates::INTERVAL * 3);
    assert!(!service.agent_updates_due(started, late));
    let on = AppSettings {
        agent_updates: true,
        ..service.settings().clone()
    };
    assert!(
        service
            .save_settings(on)
            .unwrap()
            .snapshot
            .settings
            .agent_updates
    );
    assert!(Service::open(&dir.0).settings().agent_updates);
    assert!(!service.agent_updates_due(started, started));
    assert!(service.agent_updates_due(started, after(agent_updates::STARTUP_DELAY)));
    // Not while a run is in progress, then four hours after it ended.
    assert!(service.begin_agent_updates());
    assert!(!service.agent_updates_due(started, late));
    let ended = after(Duration::from_secs(300));
    service.finish_agent_updates(ended);
    assert!(!service.agent_updates_due(started, ended + Duration::from_secs(3600)));
    assert!(service.agent_updates_due(started, ended + agent_updates::INTERVAL));
}
