//! Profiles through the service: settings, the launch plan, recents, the sync before a start
//! and the sign-in terminal. Synthetic data only.
use super::*;
use crate::detect::Fs;
use crate::launch::tests::script;
use crate::models::{AgentProfile, AppSettings, LaunchOptions, SessionMode, Snapshot, TerminalId};
use crate::testing::{windows_host, FakeFs, TempDir};
use chrono::TimeZone;

const CLAUDE_EXE: &str = r"C:\Users\max\.local\bin\claude.exe";
const CODEX_EXE: &str = r"C:\Users\max\AppData\Roaming\npm\codex.cmd";
const DEMO: &str = r"C:\Code\demo";
const SET_TEAM: &str = r"$env:CLAUDE_CONFIG_DIR='C:\Users\max\.claude-team'";
const CLEAR: &str = "Remove-Item Env:CLAUDE_CONFIG_DIR -ErrorAction SilentlyContinue";

fn at(minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 13, 9, minute, 0).unwrap()
}

fn profile(id: &str, label: &str) -> AgentProfile {
    AgentProfile {
        id: id.into(),
        label: label.into(),
    }
}

fn with_profiles(list: &[(&str, &str)]) -> AppSettings {
    let mut settings = AppSettings::default();
    settings.profiles.claude = list.iter().map(|(id, label)| profile(id, label)).collect();
    settings
}

/// A service whose Claude Code has the profiles „Team“ and „Solo“.
fn service(dir: &TempDir) -> Service {
    let mut service = Service::open(&dir.0);
    service
        .save_settings(with_profiles(&[("team", "Team"), ("solo", "Solo")]))
        .unwrap();
    service
}

fn request(agent: AgentId, env: EnvId, profile: Option<&str>) -> LaunchRequest {
    LaunchRequest {
        agent,
        env,
        folder: DEMO.into(),
        repo: None,
        options: LaunchOptions::default(),
        session: SessionMode::New,
        ports: Vec::new(),
        profile: profile.map(Into::into),
    }
}

fn fs() -> FakeFs {
    FakeFs::with(&[CLAUDE_EXE, CODEX_EXE])
        .dir(DEMO)
        .dir(r"C:\Users\max")
}

fn statements(plan: &crate::launch::LaunchPlan) -> Vec<String> {
    script(plan).split("; ").map(str::to_string).collect()
}

#[test]
fn the_snapshot_lists_the_default_profile_first_then_the_settings_order() {
    let native = Snapshot::native();
    let ids: Vec<_> = native
        .profiles
        .iter()
        .map(|p| (p.agent, p.id.as_str()))
        .collect();
    assert_eq!(ids, [(AgentId::Claude, "default")]);
    assert_eq!(native.profiles[0].label, "Standard");
    assert_eq!(native.settings.agent_profile.claude, "default");
    assert!(native.settings.profiles.claude.is_empty());

    let dir = TempDir::new("service-profiles-list");
    let mut service = service(&dir);
    let revision = service.snapshot().revision;
    service.set_profile_homes(crate::profiles::Homes::of(&windows_host()));
    let snapshot = service.snapshot();
    assert!(snapshot.revision > revision);
    let shown: Vec<_> = snapshot
        .profiles
        .iter()
        .map(|p| (p.id.as_str(), p.label.as_str(), p.dir.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("default", "Standard", r"C:\Users\max\.claude"),
            ("team", "Team", r"C:\Users\max\.claude-team"),
            ("solo", "Solo", r"C:\Users\max\.claude-solo"),
        ]
    );
    assert!(snapshot.profiles.iter().all(|p| !p.signed_in));
}

#[test]
fn saving_validates_profiles_and_a_removed_default_falls_back() {
    let dir = TempDir::new("service-profiles-save");
    let mut service = Service::open(&dir.0);
    let mut settings = with_profiles(&[("team", " Team "), ("solo", "Solo")]);
    settings.agent_profile.claude = "team".into();
    let saved = service.save_settings(settings.clone()).unwrap();
    assert!(saved.profiles_changed);
    assert_eq!(saved.snapshot.settings.profiles.claude[0].label, "Team");
    assert_eq!(saved.snapshot.settings.agent_profile.claude, "team");
    let host = windows_host();
    let prepared = service
        .prepare_launch(
            &request(AgentId::Claude, EnvId::Local, Some("team")),
            &host,
            &fs(),
            "n",
        )
        .unwrap();
    let recent = service.record_launch(&prepared.request, at(1));
    assert_eq!(recent.profile.as_deref(), Some("team"));
    // Saving the same list again is no profile change.
    let again = service.save_settings(service.settings().clone()).unwrap();
    assert!(!again.profiles_changed);
    // What the profiles share on top of the recipe is a profile change too: it is synced.
    let mut sharing = service.settings().clone();
    sharing.profile_share.claude.dirs = vec![" notes ".into()];
    let saved = service.save_settings(sharing.clone()).unwrap();
    assert!(saved.profiles_changed);
    assert_eq!(saved.snapshot.settings.profile_share.claude.dirs, ["notes"]);
    assert_eq!(service.profile_plan().share.claude.dirs, ["notes"]);
    assert!(!service.save_settings(sharing).unwrap().profiles_changed);
    let mut refused = service.settings().clone();
    refused.profile_share.claude.files = vec![".credentials.json".into()];
    assert_eq!(
        service.save_settings(refused).unwrap_err(),
        "Name „.credentials.json“ für Claude Code ist ungültig."
    );
    let invalid = service.save_settings(with_profiles(&[("Team", "Team")]));
    assert!(invalid
        .unwrap_err()
        .starts_with("Profil-ID für Claude Code ist ungültig"));
    // Removing the profile: the default one takes over, the recent stays as it is …
    let mut without = with_profiles(&[("solo", "Solo")]);
    without.agent_profile.claude = "team".into();
    let saved = service.save_settings(without).unwrap();
    assert!(saved.profiles_changed);
    assert_eq!(saved.snapshot.settings.agent_profile.claude, "default");
    assert_eq!(saved.snapshot.recents[0].profile.as_deref(), Some("team"));
    // … and no longer starts.
    let restart = request(AgentId::Claude, EnvId::Local, Some("team"));
    assert_eq!(
        service.prepare_launch(&restart, &host, &fs(), "n"),
        Err("Profil nicht gefunden.".into())
    );
    let reloaded = Service::open(&dir.0);
    assert_eq!(
        reloaded.settings().profiles.claude,
        [profile("solo", "Solo")]
    );
    assert_eq!(
        reloaded.snapshot().recents[0].profile.as_deref(),
        Some("team")
    );
}

#[test]
fn a_local_start_points_the_cli_at_its_profile() {
    let dir = TempDir::new("service-profiles-launch");
    let service = service(&dir);
    let host = windows_host();
    let launch = |profile: Option<&str>| {
        service
            .prepare_launch(
                &request(AgentId::Claude, EnvId::Local, profile),
                &host,
                &fs(),
                "n",
            )
            .unwrap()
    };
    let team = launch(Some("team"));
    assert_eq!(team.request.profile.as_deref(), Some("team"));
    let lines = statements(&team.plan);
    let env = lines
        .iter()
        .position(|line| line == SET_TEAM)
        .expect("env line");
    let call = lines
        .iter()
        .position(|line| line.contains(CLAUDE_EXE))
        .unwrap();
    assert!(env < call, "{lines:?}");
    assert!(!lines.iter().any(|line| line == CLEAR));
    assert_eq!(
        statements(&team.fallback.clone().unwrap_or(team.plan.clone())),
        lines
    );
    // The default profile: the CLI's own home, whatever the terminal's environment holds.
    for default in [None, Some("default"), Some(" default "), Some("")] {
        let plain = launch(default);
        assert_eq!(plain.request.profile, None, "{default:?}");
        let lines = statements(&plain.plan);
        assert!(lines.iter().any(|line| line == CLEAR), "{lines:?}");
        assert!(!script(&plain.plan).contains("$env:CLAUDE_CONFIG_DIR"));
    }
    for unknown in ["gone", "Team", "..\\x", "team;calc"] {
        let refused = service.prepare_launch(
            &request(AgentId::Claude, EnvId::Local, Some(unknown)),
            &host,
            &fs(),
            "n",
        );
        assert_eq!(refused, Err("Profil nicht gefunden.".into()), "{unknown}");
    }
    // An agent without profiles has only its own home.
    let codex = service.prepare_launch(
        &request(AgentId::Codex, EnvId::Local, Some("team")),
        &host,
        &fs(),
        "n",
    );
    assert_eq!(codex, Err("Profil nicht gefunden.".into()));
    let codex = service
        .prepare_launch(
            &request(AgentId::Codex, EnvId::Local, None),
            &host,
            &fs(),
            "n",
        )
        .unwrap();
    assert!(!script(&codex.plan).contains("CONFIG_DIR"));
    // Without a user profile there is no profile directory.
    let mut homeless = windows_host();
    homeless.user_profile = None;
    let refused = service.prepare_launch(
        &request(AgentId::Claude, EnvId::Local, Some("team")),
        &homeless,
        &FakeFs::with(&[r"C:\Users\max\AppData\Roaming\npm\claude.cmd"]).dir(DEMO),
        "n",
    );
    assert!(refused
        .unwrap_err()
        .starts_with("Der Profilordner ist nicht verfügbar"));
}

#[test]
fn without_an_extra_profile_the_script_is_as_before() {
    let dir = TempDir::new("service-profiles-plain");
    let service = Service::open(&dir.0);
    let prepared = service
        .prepare_launch(
            &request(AgentId::Claude, EnvId::Local, Some("default")),
            &windows_host(),
            &fs(),
            "n",
        )
        .unwrap();
    assert_eq!(prepared.request.profile, None);
    assert!(!script(&prepared.plan).contains("CLAUDE_CONFIG_DIR"));
}

#[test]
fn a_docker_start_ignores_the_profile() {
    const DOCKER: &str = r"C:\Program Files\Docker\Docker\resources\bin\docker.exe";
    let dir = TempDir::new("service-profiles-docker");
    let mut service = service(&dir);
    let host = windows_host();
    let fs = FakeFs::with(&[DOCKER]).dir(DEMO);
    let plain = service
        .prepare_launch(
            &request(AgentId::Claude, EnvId::Docker, None),
            &host,
            &fs,
            "n",
        )
        .unwrap();
    // Also one that does not exist: the container has its own home.
    for profile in ["team", "gone"] {
        let prepared = service
            .prepare_launch(
                &request(AgentId::Claude, EnvId::Docker, Some(profile)),
                &host,
                &fs,
                "n",
            )
            .unwrap();
        assert_eq!(prepared.request.profile, None);
        assert_eq!(script(&prepared.plan), script(&plain.plan));
        assert!(!script(&prepared.plan).contains(".claude-team"));
        let recent = service.record_launch(&prepared.request, at(1));
        assert_eq!(recent.profile, None);
        let linker: Box<dyn crate::profiles::Linker> = Box::new(crate::testing::TestLinker);
        let synced = service.sync_for_launch(&prepared.request, linker.as_ref(), at(1));
        assert_eq!(synced, Ok(None));
    }
}

#[test]
fn a_recent_keeps_its_id_and_takes_the_profile_of_the_latest_start() {
    let dir = TempDir::new("service-profiles-recents");
    let mut service = service(&dir);
    let host = windows_host();
    let mut start = |profile: Option<&str>, minute: u32| {
        let prepared = service
            .prepare_launch(
                &request(AgentId::Claude, EnvId::Local, profile),
                &host,
                &fs(),
                "n",
            )
            .unwrap();
        service.record_launch(&prepared.request, at(minute))
    };
    let first = start(Some("team"), 1);
    let second = start(Some("solo"), 2);
    let third = start(None, 3);
    assert_eq!(first.profile.as_deref(), Some("team"));
    assert_eq!(second.profile.as_deref(), Some("solo"));
    assert_eq!((third.profile.as_deref(), third.starts), (None, 3));
    assert_eq!(first.id, second.id);
    assert_eq!(second.id, third.id);
    let json = serde_json::to_value(&third).unwrap();
    assert!(
        json.get("profile").is_none(),
        "absent for the default profile"
    );
    assert_eq!(serde_json::to_value(&second).unwrap()["profile"], "solo");
    assert_eq!(service.snapshot().recents.len(), 1);
    // An undo keeps a valid profile and drops anything else.
    let removed = second.clone();
    service.remove_recent(&third.id).unwrap();
    let restored = service.restore_recent(removed).unwrap();
    assert_eq!(restored.recents[0].profile.as_deref(), Some("solo"));
    service.remove_recent(&third.id).unwrap();
    let tampered = crate::models::RecentSession {
        profile: Some("..\\..\\x".into()),
        ..second
    };
    let restored = service.restore_recent(tampered).unwrap();
    assert_eq!(restored.recents[0].profile, None);
}

#[test]
fn the_sign_in_terminal_runs_auth_login_in_the_profiles_environment() {
    let dir = TempDir::new("service-profiles-login");
    let service = service(&dir);
    let host = windows_host();
    let fs = fs();
    let (plan, fallback) = service
        .prepare_login(AgentId::Claude, "team", &host, &fs)
        .unwrap();
    assert_eq!(plan.title, "Claude Code · Team anmelden");
    assert_eq!(plan.cwd, r"C:\Users\max");
    assert_eq!(plan.terminal, TerminalId::Console);
    assert!(fallback.is_none());
    assert_eq!(
        statements(&plan),
        [
            "$Host.UI.RawUI.WindowTitle='Claude Code · Team anmelden'",
            SET_TEAM,
            &format!("& '{CLAUDE_EXE}' 'auth' 'login'"),
        ]
    );
    // The terminal stays open: no exit after the command.
    assert!(plan.args.contains(&"-NoExit".to_string()));
    assert!(!script(&plan).contains("Exit("));
    assert_eq!(fs.created(), [r"C:\Users\max\.claude-team"]);
    // The default profile signs in to the CLI's own home.
    let (plan, _) = service
        .prepare_login(AgentId::Claude, "default", &host, &fs)
        .unwrap();
    assert_eq!(plan.title, "Claude Code · Standard anmelden");
    assert_eq!(statements(&plan)[1], CLEAR);
    assert_eq!(fs.created().len(), 1);
    let refused = |agent, profile: &str, fs: &dyn Fs| {
        service
            .prepare_login(agent, profile, &host, fs)
            .unwrap_err()
    };
    assert_eq!(
        refused(AgentId::Claude, "gone", &fs),
        "Profil nicht gefunden."
    );
    assert_eq!(
        refused(AgentId::Codex, "default", &fs),
        "Codex unterstützt keine Profile."
    );
    assert_eq!(
        refused(AgentId::Claude, "team", &FakeFs::default()),
        "Claude Code ist nicht installiert."
    );
    // A directory that is someone else's gets no sign-in: the credentials would make it
    // look like Claude Code's own.
    let mut foreign = self::fs();
    foreign.foreign.insert(r"C:\Users\max\.claude-team".into());
    assert_eq!(
        refused(AgentId::Claude, "team", &foreign),
        "Der Profilordner gehört nicht zu Claude Code."
    );
    assert!(foreign.created().is_empty(), "nothing is created either");
    assert!(service
        .prepare_login(AgentId::Claude, "solo", &host, &foreign)
        .is_ok());
    assert!(service
        .prepare_login(AgentId::Claude, "default", &host, &foreign)
        .is_ok());
    let read_only = FakeFs {
        read_only: true,
        ..self::fs()
    };
    assert_eq!(
        refused(AgentId::Claude, "team", &read_only),
        "Der Profilordner konnte nicht angelegt werden."
    );
}

/// The sync before a start runs without the service lock; what changed meanwhile is caught
/// before the terminal opens.
#[test]
fn a_prepared_start_is_confirmed_before_it_spawns() {
    let dir = TempDir::new("service-profiles-confirm");
    let mut service = service(&dir);
    let host = windows_host();
    let prepared = service
        .prepare_launch(
            &request(AgentId::Claude, EnvId::Local, Some("team")),
            &host,
            &fs(),
            "n",
        )
        .unwrap();
    assert_eq!(service.confirm_launch(&prepared.request), Ok(()));
    // The profile was removed meanwhile.
    service
        .save_settings(with_profiles(&[("solo", "Solo")]))
        .unwrap();
    assert_eq!(
        service.confirm_launch(&prepared.request),
        Err("Profil nicht gefunden.".into())
    );
    // The default profile and a container are not affected by the profile list.
    let plain = request(AgentId::Claude, EnvId::Local, None);
    assert_eq!(service.confirm_launch(&plain), Ok(()));
    // An update of the agent began meanwhile: a local start waits, a container does not.
    service.set_agent_update(crate::agent_updates::status(
        AgentId::Claude,
        crate::models::AgentUpdateState::Running,
        at(1),
    ));
    assert_eq!(
        service.confirm_launch(&plain),
        Err("Claude Code wird gerade aktualisiert.".into())
    );
    let docker = request(AgentId::Claude, EnvId::Docker, None);
    assert_eq!(service.confirm_launch(&docker), Ok(()));
}

mod schedule;
mod sync;
