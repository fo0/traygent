use super::*;
use crate::detect::Candidate;
use crate::launch::tests::script;
use crate::models::{LaunchOptions, PerAgent, PermissionId, SessionMode, TerminalId};
use crate::testing::{windows_host, FakeFs, TempDir, NPM};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fixtures/snapshot.default.json"
);
const CLAUDE_EXE: &str = r"C:\Users\max\.local\bin\claude.exe";
const WT: &str = r"C:\Users\max\AppData\Local\Microsoft\WindowsApps\wt.exe";
const DOCKER: &str = r"C:\Program Files\Docker\Docker\resources\bin\docker.exe";

/// The golden fixture the panel test checks against `Snapshot` (src/lib/types.ts).
/// `TRAYGENT_UPDATE_FIXTURES=1` rewrites it; run prettier on it afterwards.
#[test]
fn the_default_native_snapshot_matches_the_golden_fixture() {
    let snapshot = serde_json::to_value(Snapshot::native()).unwrap();
    if std::env::var_os("TRAYGENT_UPDATE_FIXTURES").is_some() {
        let mut text = serde_json::to_string_pretty(&Snapshot::native()).unwrap();
        text.push('\n');
        std::fs::write(FIXTURE, text).unwrap();
    }
    let text = std::fs::read_to_string(FIXTURE).expect("fixtures/snapshot.default.json");
    assert!(text.ends_with("}\n"));
    let fixture: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        fixture, snapshot,
        "rerun with TRAYGENT_UPDATE_FIXTURES=1, then prettier"
    );
}

#[test]
fn the_native_snapshot_before_detection() {
    let snapshot = Snapshot::native();
    assert!(!snapshot.demo && !snapshot.detecting);
    assert_eq!(snapshot.checked_at, None);
    assert!(snapshot.recents.is_empty());
    assert_eq!(snapshot.catalog.len(), 3);
    assert_eq!(snapshot.agents.len(), 6);
    assert!(snapshot
        .agents
        .iter()
        .all(|s| s.state == ProbeState::Unknown));
    let envs: Vec<_> = snapshot
        .environments
        .iter()
        .map(|e| (e.env, e.state))
        .collect();
    assert_eq!(
        envs,
        [
            (EnvId::Local, ProbeState::Ready),
            (EnvId::Docker, ProbeState::Unknown),
            (EnvId::Wsl, ProbeState::Unavailable)
        ]
    );
    assert_eq!((snapshot.revision, snapshot.notice.as_deref()), (0, None));
    let json = serde_json::to_value(&snapshot).unwrap();
    assert!(json.get("checkedAt").is_none());
    assert!(json.get("notice").is_none());
    assert_eq!(json["revision"], 0);
}

/// Ends a detection that must not rerun and hands back its snapshot.
fn finished(service: &mut Service, detection: Option<Detection>) -> Snapshot {
    match service.finish_detection(detection) {
        DetectionEnd::Finished(snapshot) => *snapshot,
        DetectionEnd::Rerun => panic!("unexpected rerun"),
    }
}

fn request(agent: AgentId, env: EnvId, folder: &str) -> LaunchRequest {
    LaunchRequest {
        agent,
        env,
        folder: folder.into(),
        repo: None,
        options: LaunchOptions::default(),
        session: SessionMode::New,
        ports: Vec::new(),
    }
}

fn detection(agent_path: Option<(AgentId, &str, bool)>) -> Detection {
    let mut clis = PerAgent::<Option<Candidate>>::default();
    if let Some((agent, path, shim)) = agent_path {
        *clis.get_mut(agent) = Some(Candidate {
            path: path.into(),
            shim,
        });
    }
    let (agents, environments) = initial_statuses();
    Detection {
        agents,
        environments,
        clis,
        docker: None,
        checked_at: "2026-11-03T09:30:00Z".into(),
    }
}

#[test]
fn settings_are_validated_saved_and_reloaded() {
    let dir = TempDir::new("service-settings");
    let mut service = Service::open(&dir.0);
    let mut settings = AppSettings {
        terminal: TerminalId::Console,
        shared_window: true,
        ..AppSettings::default()
    };
    settings.agents.get_mut(AgentId::Claude).model = " sonnet ".into();
    let snapshot = service.save_settings(settings).unwrap().snapshot;
    assert_eq!(snapshot.settings.terminal, TerminalId::Console);
    assert!(snapshot.settings.shared_window);
    assert_eq!(snapshot.settings.agents.claude.model, "sonnet");
    let reloaded = Service::open(&dir.0);
    assert_eq!(reloaded.settings(), &snapshot.settings);
    let invalid = AppSettings {
        default_env: EnvId::Wsl,
        ..AppSettings::default()
    };
    assert_eq!(
        service.save_settings(invalid),
        Err("WSL ist noch nicht verfügbar.".into())
    );
    let none = AppSettings {
        environments: Vec::new(),
        ..AppSettings::default()
    };
    assert_eq!(
        service.save_settings(none),
        Err("Mindestens eine Umgebung muss aktiv sein.".into())
    );
    assert_eq!(service.settings().terminal, TerminalId::Console);
}

/// A tampered `state.json` neither reaches a launch nor blocks the next save (S1/F8).
#[test]
fn tampered_settings_on_disk_fall_back_per_field_and_saves_still_work() {
    let dir = TempDir::new("service-tampered");
    let tampered = serde_json::json!({
        "version": 1,
        "settings": {
            "defaultEnv": "wsl",
            "terminal": "console",
            "agents": { "claude": { "model": "--x", "effort": "high", "extraArgs": "a\"b" } },
            "docker": {
                "images": { "claude": "--privileged", "codex": "-eX=\u{a0}\" \"--privileged", "opencode": "ghcr.io/me/oc:1" },
                "ghToken": true
            }
        }
    });
    std::fs::write(dir.0.join("state.json"), tampered.to_string()).unwrap();
    let mut service = Service::open(&dir.0);
    let settings = service.settings().clone();
    assert_eq!(settings.default_env, EnvId::Local);
    assert_eq!(settings.terminal, TerminalId::Console);
    let claude = settings.agents.get(AgentId::Claude);
    assert_eq!(
        (
            claude.model.as_str(),
            claude.effort.as_str(),
            claude.extra_args.as_str()
        ),
        ("default", "high", "")
    );
    assert_eq!(
        settings.docker.images.claude,
        crate::settings::DEFAULT_IMAGE
    );
    assert_eq!(settings.docker.images.codex, crate::settings::DEFAULT_IMAGE);
    assert_eq!(settings.docker.images.opencode, "ghcr.io/me/oc:1");
    assert!(settings.docker.gh_token);
    assert!(service.save_settings(settings).is_ok());
}

#[test]
fn a_read_only_state_refuses_changes_but_still_launches() {
    let dir = TempDir::new("service-readonly");
    std::fs::write(dir.0.join("state.json"), r#"{"version":9}"#).unwrap();
    let mut service = Service::open(&dir.0);
    assert_eq!(service.notice(), Some(store::NEWER_VERSION));
    assert_eq!(
        service.snapshot().notice.as_deref(),
        Some(store::NEWER_VERSION)
    );
    assert!(
        store::NEWER_VERSION.starts_with("Einstellungen und Sessions werden nicht gespeichert: ")
    );
    assert_eq!(
        service.save_settings(AppSettings::default()),
        Err(store::NEWER_VERSION.into())
    );
    let recent = service.record_launch(&request(AgentId::Codex, EnvId::Local, r"C:\x"), Utc::now());
    assert_eq!(service.snapshot().recents, vec![recent.clone()]);
    assert_eq!(
        service.remove_recent(&recent.id),
        Err(store::NEWER_VERSION.into())
    );
    assert_eq!(
        std::fs::read_to_string(dir.0.join("state.json")).unwrap(),
        r#"{"version":9}"#
    );
    let mut fallback = Service::read_only(store::NO_APP_DATA);
    assert!(fallback.save_settings(AppSettings::default()).is_err());
    assert_eq!(
        fallback.snapshot().notice.as_deref(),
        Some(store::NO_APP_DATA)
    );
}

#[test]
fn a_failed_save_after_a_start_becomes_the_notice_until_a_save_works() {
    let dir = TempDir::new("service-save-failed");
    let mut service = Service::open(&dir.0);
    assert_eq!(service.snapshot().notice, None);
    // A directory named state.json blocks the atomic rename.
    std::fs::create_dir(dir.0.join("state.json")).unwrap();
    let recent =
        service.record_launch(&request(AgentId::Claude, EnvId::Local, r"C:\a"), Utc::now());
    let snapshot = service.snapshot();
    assert_eq!(snapshot.recents, vec![recent]);
    assert_eq!(snapshot.notice.as_deref(), Some(SAVE_FAILED));
    std::fs::remove_dir(dir.0.join("state.json")).unwrap();
    service.save_settings(AppSettings::default()).unwrap();
    assert_eq!(service.snapshot().notice, None);
}

#[test]
fn every_state_change_bumps_the_revision() {
    let dir = TempDir::new("service-revision");
    let mut service = Service::open(&dir.0);
    let mut last = service.snapshot().revision;
    assert_eq!(last, 0);
    let mut bumped = |snapshot: Snapshot, what: &str| {
        assert!(
            snapshot.revision > last,
            "{what}: {} <= {last}",
            snapshot.revision
        );
        last = snapshot.revision;
    };
    let settings = AppSettings {
        terminal: TerminalId::Console,
        ..AppSettings::default()
    };
    bumped(
        service.save_settings(settings).unwrap().snapshot,
        "settings",
    );
    let recent = service.record_launch(&request(AgentId::Codex, EnvId::Local, r"C:\a"), Utc::now());
    bumped(service.snapshot(), "recent added");
    bumped(service.remove_recent(&recent.id).unwrap(), "recent removed");
    bumped(service.restore_recent(recent).unwrap(), "recent restored");
    assert!(service.begin_detection());
    bumped(service.snapshot(), "detection started");
    bumped(
        finished(&mut service, Some(detection(None))),
        "detection finished",
    );
    // Nothing changed: same revision.
    let unchanged = service.remove_recent("unknown").unwrap();
    assert_eq!(unchanged.revision, service.snapshot().revision);
    assert!(service
        .save_settings(AppSettings {
            default_env: EnvId::Wsl,
            ..AppSettings::default()
        })
        .is_err());
    assert_eq!(service.snapshot().revision, unchanged.revision);
}

/// New Docker images invalidate the Docker statuses: a detection starts, or a running one
/// is marked stale and reruns instead of publishing results for the old images (F7).
#[test]
fn changed_docker_images_restart_the_detection() {
    let dir = TempDir::new("service-images");
    let mut service = Service::open(&dir.0);
    let mut settings = AppSettings {
        terminal: TerminalId::Console,
        ..AppSettings::default()
    };
    let saved = service.save_settings(settings.clone()).unwrap();
    assert!(!saved.start_detection && !saved.snapshot.detecting);
    *settings.docker.images.get_mut(AgentId::Codex) = "ghcr.io/me/codex:1".into();
    let saved = service.save_settings(settings.clone()).unwrap();
    assert!(saved.start_detection && saved.snapshot.detecting);
    assert!(!service.begin_detection());
    // Changed again while that run probes: no second run, the first one goes stale.
    *settings.docker.images.get_mut(AgentId::Codex) = "ghcr.io/me/codex:2".into();
    let saved = service.save_settings(settings).unwrap();
    assert!(!saved.start_detection && saved.snapshot.detecting);
    assert_eq!(
        service.finish_detection(Some(detection(None))),
        DetectionEnd::Rerun
    );
    assert!(service.snapshot().detecting);
    assert_eq!(service.snapshot().checked_at, None);
    let done = finished(&mut service, Some(detection(None)));
    assert!(!done.detecting);
    assert_eq!(done.checked_at.as_deref(), Some("2026-11-03T09:30:00Z"));
    // A thread that never started clears the flag too.
    assert!(service.begin_detection());
    service.abort_detection();
    assert!(!service.snapshot().detecting);
    assert!(service.begin_detection());
}

#[test]
fn recents_are_recorded_removed_and_restored_on_disk() {
    let dir = TempDir::new("service-recents");
    let mut service = Service::open(&dir.0);
    let first = service.record_launch(&request(AgentId::Claude, EnvId::Local, r"C:\a"), Utc::now());
    let second = service.record_launch(
        &request(AgentId::Claude, EnvId::Docker, r"C:\a"),
        Utc::now(),
    );
    assert_eq!(Service::open(&dir.0).snapshot().recents.len(), 2);
    let after_remove = service.remove_recent(&first.id).unwrap();
    assert_eq!(after_remove.recents, vec![second.clone()]);
    assert_eq!(service.remove_recent("unknown").unwrap().recents.len(), 1);
    let restored = service.restore_recent(first.clone()).unwrap();
    assert_eq!(restored.recents.len(), 2);
    assert_eq!(Service::open(&dir.0).snapshot().recents.len(), 2);
    let mut broken = first.clone();
    broken.folder = "C:\\a\u{1}".into();
    assert!(service.restore_recent(broken).is_err());
    let mut wsl = first;
    wsl.env = EnvId::Wsl;
    assert_eq!(
        service.restore_recent(wsl),
        Err("WSL ist noch nicht verfügbar.".into())
    );
}

#[test]
fn detection_runs_once_at_a_time_and_keeps_results_on_failure() {
    let dir = TempDir::new("service-detect");
    let mut service = Service::open(&dir.0);
    assert!(service.begin_detection());
    assert!(!service.begin_detection());
    assert!(service.snapshot().detecting);
    let done = finished(&mut service, Some(detection(None)));
    assert!(!done.detecting);
    assert_eq!(done.checked_at.as_deref(), Some("2026-11-03T09:30:00Z"));
    assert!(service.begin_detection());
    let failed = finished(&mut service, None);
    assert_eq!(failed.checked_at.as_deref(), Some("2026-11-03T09:30:00Z"));
}

#[test]
fn prepare_launch_checks_the_folder_and_falls_back_to_a_console() {
    let dir = TempDir::new("service-launch");
    let service = Service::open(&dir.0);
    let host = windows_host();
    let fs = FakeFs::with(&[CLAUDE_EXE, WT]).dir(r"C:\Code\demo");
    let missing = service.prepare_launch(
        &request(AgentId::Claude, EnvId::Local, r"C:\nope"),
        &host,
        &fs,
        "abc",
    );
    assert_eq!(missing, Err("Ordner nicht gefunden.".into()));
    let prepared = service
        .prepare_launch(
            &request(AgentId::Claude, EnvId::Local, r"C:\Code\demo\"),
            &host,
            &fs,
            "abc",
        )
        .unwrap();
    assert_eq!(prepared.request.folder, r"C:\Code\demo");
    assert_eq!(prepared.plan.program, WT);
    let fallback = prepared.fallback.unwrap();
    assert_eq!(fallback.terminal, TerminalId::Console);
    assert_eq!(fallback.program, host.powershell());
    assert!(script(&fallback).contains(CLAUDE_EXE));
    let not_installed = service.prepare_launch(
        &request(AgentId::Codex, EnvId::Local, r"C:\Code\demo"),
        &host,
        &fs,
        "abc",
    );
    assert_eq!(not_installed, Err("Codex ist nicht installiert.".into()));
}

#[test]
fn a_detected_cli_wins_and_a_rejected_v1_is_skipped() {
    let dir = TempDir::new("service-resolve");
    let mut service = Service::open(&dir.0);
    let host = windows_host();
    let v1 = r"C:\Users\max\.opencode\bin\opencode.exe";
    let v2 = format!(r"{NPM}\opencode2.cmd");
    let fs = FakeFs::with(&[v1, &v2]).dir(r"C:\x");
    let req = request(AgentId::Opencode, EnvId::Local, r"C:\x");
    service.begin_detection();
    finished(
        &mut service,
        Some(detection(Some((AgentId::Opencode, &v2, true)))),
    );
    let plan = service.prepare_launch(&req, &host, &fs, "n").unwrap().plan;
    assert!(script(&plan).contains(&v2));
    let mut outdated = detection(None);
    let local = outdated
        .agents
        .iter_mut()
        .find(|s| s.agent == AgentId::Opencode && s.env == EnvId::Local)
        .unwrap();
    local.state = ProbeState::Missing;
    local.path = Some(v1.into());
    service.begin_detection();
    finished(&mut service, Some(outdated));
    let plan = service.prepare_launch(&req, &host, &fs, "n").unwrap().plan;
    assert!(!script(&plan).contains(v1));
    assert!(script(&plan).contains(&v2));
}

#[test]
fn docker_launches_resolve_docker_gh_and_the_gitconfig() {
    let dir = TempDir::new("service-docker");
    let mut service = Service::open(&dir.0);
    let mut settings = AppSettings {
        terminal: TerminalId::Console,
        ..AppSettings::default()
    };
    settings.docker.gh_token = true;
    settings.agents.get_mut(AgentId::Claude).permission = PermissionId::Ask;
    service.save_settings(settings).unwrap();
    let host = windows_host();
    let gh = r"C:\Program Files\GitHub CLI\gh.exe";
    let gitconfig = r"C:\Users\max\.gitconfig";
    let fs = FakeFs::with(&[DOCKER, gh, gitconfig]).dir(r"C:\x");
    let mut req = request(AgentId::Claude, EnvId::Docker, r"C:\x");
    req.options = service.settings().agents.claude.clone();
    let prepared = service.prepare_launch(&req, &host, &fs, "n0nce1").unwrap();
    assert_eq!(prepared.fallback, None);
    let script = script(&prepared.plan);
    assert!(script.contains(&format!("& '{DOCKER}' 'run'")));
    assert!(script.contains(&format!("(& '{gh}' auth token 2>$null)")));
    assert!(script.contains(&format!("'{gitconfig}:/home/node/.gitconfig:ro'")));
    assert!(script.contains("'--name' 'traygent-claude-x-"));
    assert!(script.contains("-n0nce1'"));
    let no_docker = service.prepare_launch(&req, &host, &FakeFs::default().dir(r"C:\x"), "n");
    assert_eq!(no_docker, Err("Docker ist nicht installiert.".into()));
}

#[test]
fn install_terminals_start_in_the_user_profile() {
    let dir = TempDir::new("service-install");
    let service = Service::open(&dir.0);
    let host = windows_host();
    let fs = FakeFs::with(&[WT]).dir(r"C:\Users\max");
    let (plan, fallback) = service.prepare_install(AgentId::Codex, &host, &fs).unwrap();
    assert_eq!(plan.program, WT);
    assert_eq!(plan.cwd, r"C:\Users\max");
    assert!(script(&plan).contains("irm https://chatgpt.com/codex/install.ps1 | iex"));
    assert_eq!(fallback.unwrap().terminal, TerminalId::Console);
    let (plan, _) = service
        .prepare_install(AgentId::Codex, &host, &FakeFs::default())
        .unwrap();
    assert_eq!(plan.cwd, "C:\\");
}

#[test]
fn nonces_are_six_hex_digits() {
    let nonce = nonce();
    assert_eq!(nonce.len(), 6);
    assert!(nonce.chars().all(|c| c.is_ascii_hexdigit()));
}

mod ports;

mod recent_edit;

mod repo;
