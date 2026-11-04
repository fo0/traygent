//! Repository starts through the service: the derived folder, reuse or clone, the clone root
//! setting and the recorded recent (docs/SPEC.md → Repositories).
use super::*;
use crate::detect::Host;
use crate::models::{GithubAccount, GithubSource, RepoRef};

const GIT: &str = r"C:\Program Files\Git\cmd\git.exe";
const ROOT: &str = r"C:\Users\max\Traygent";
const FOLDER: &str = r"C:\Users\max\Traygent\fo0\traygent";

fn repo_request(agent: AgentId, env: EnvId, name: &str) -> LaunchRequest {
    LaunchRequest {
        repo: Some(RepoRef {
            owner: "fo0".into(),
            name: name.into(),
        }),
        // The composer's start: no folder, so it is derived from the clone root.
        ..request(agent, env, "")
    }
}

/// ▶ on a recent started from a repository: its folder travels with the repository.
fn restart_request(folder: &str) -> LaunchRequest {
    LaunchRequest {
        folder: folder.into(),
        ..repo_request(AgentId::Claude, EnvId::Local, "traygent")
    }
}

/// A clone under the clone root of an earlier release (#40).
const OLD_FOLDER: &str = r"C:\Users\max\source\repos\fo0\traygent";

#[test]
fn a_restart_reuses_the_recent_folder_while_it_exists() {
    let dir = TempDir::new("service-repo-restart-reuse");
    let mut service = Service::open(&dir.0);
    let fs = FakeFs::with(&[CLAUDE_EXE, WT]).dir(OLD_FOLDER);
    let req = restart_request(&format!("{OLD_FOLDER}\\"));
    let prepared = service
        .prepare_launch(&req, &windows_host(), &fs, "n")
        .unwrap();
    assert_eq!(prepared.request.folder, OLD_FOLDER);
    assert_eq!(prepared.plan.cwd, OLD_FOLDER);
    assert!(!script(&prepared.plan).contains("'clone'"));
    assert!(fs.created().is_empty());
    let recent = service.record_launch(&prepared.request, Utc::now());
    assert_eq!(recent.folder, OLD_FOLDER);
    assert_eq!(recent.repo, req.repo);
}

#[test]
fn a_restart_whose_folder_is_gone_clones_into_the_current_root() {
    let dir = TempDir::new("service-repo-restart-gone");
    let service = Service::open(&dir.0);
    let fs = FakeFs::with(&[CLAUDE_EXE, WT, GIT]);
    let prepared = service
        .prepare_launch(&restart_request(OLD_FOLDER), &windows_host(), &fs, "n")
        .unwrap();
    assert_eq!(prepared.request.folder, FOLDER);
    assert_eq!(prepared.plan.cwd, ROOT);
    assert!(script(&prepared.plan).contains(&format!(
        "'clone' '--' 'https://github.com/fo0/traygent.git' '{FOLDER}'"
    )));
    assert_eq!(fs.created(), [ROOT]);
    // A clone already under the current root is reused.
    let cloned = FakeFs::with(&[CLAUDE_EXE, WT]).dir(FOLDER);
    let prepared = service
        .prepare_launch(&restart_request(OLD_FOLDER), &windows_host(), &cloned, "n")
        .unwrap();
    assert_eq!(prepared.request.folder, FOLDER);
    assert!(!script(&prepared.plan).contains("'clone'"));
    assert!(cloned.created().is_empty());
}

#[test]
fn a_restart_with_an_invalid_folder_is_refused() {
    let dir = TempDir::new("service-repo-restart-invalid");
    let service = Service::open(&dir.0);
    let fs = FakeFs::with(&[CLAUDE_EXE, GIT]);
    let error = service
        .prepare_launch(&restart_request("relativ"), &windows_host(), &fs, "n")
        .unwrap_err();
    assert_eq!(error, "Bitte einen absoluten Ordnerpfad wählen.");
    assert!(fs.created().is_empty());
}

#[test]
fn an_existing_clone_folder_is_reused_without_git() {
    let dir = TempDir::new("service-repo-reuse");
    let service = Service::open(&dir.0);
    let fs = FakeFs::with(&[CLAUDE_EXE, WT]).dir(FOLDER);
    let req = repo_request(AgentId::Claude, EnvId::Local, "traygent.git");
    let prepared = service
        .prepare_launch(&req, &windows_host(), &fs, "n")
        .unwrap();
    assert_eq!(prepared.request.folder, FOLDER);
    assert_eq!(prepared.request.repo.as_ref().unwrap().name, "traygent");
    assert_eq!(prepared.plan.cwd, FOLDER);
    assert!(!script(&prepared.plan).contains("'clone'"));
    assert!(fs.created().is_empty());
    assert_eq!(prepared.fallback.unwrap().cwd, FOLDER);
}

#[test]
fn a_missing_clone_folder_is_cloned_from_the_created_clone_root() {
    let dir = TempDir::new("service-repo-clone");
    let service = Service::open(&dir.0);
    let fs = FakeFs::with(&[CLAUDE_EXE, WT, GIT]);
    let req = repo_request(AgentId::Claude, EnvId::Local, "traygent");
    let prepared = service
        .prepare_launch(&req, &windows_host(), &fs, "n")
        .unwrap();
    assert_eq!(prepared.request.folder, FOLDER);
    assert_eq!(
        (prepared.plan.program.as_str(), prepared.plan.cwd.as_str()),
        (WT, ROOT)
    );
    assert_eq!(prepared.plan.args[..3], ["new-tab", "-d", ROOT]);
    let script = script(&prepared.plan);
    assert!(script.contains(&format!(
        "& '{GIT}' 'clone' '--' 'https://github.com/fo0/traygent.git' '{FOLDER}'; if ($LASTEXITCODE -ne 0) {{ throw "
    )));
    let fallback = prepared.fallback.unwrap();
    assert_eq!(
        (fallback.terminal, fallback.cwd.as_str()),
        (TerminalId::Console, ROOT)
    );
    assert_eq!(fs.created(), [ROOT]);
}

#[test]
fn a_docker_clone_resolves_git_and_docker() {
    let dir = TempDir::new("service-repo-docker");
    let service = Service::open(&dir.0);
    let fs = FakeFs::with(&[DOCKER, GIT]);
    let req = repo_request(AgentId::Claude, EnvId::Docker, "traygent");
    let prepared = service
        .prepare_launch(&req, &windows_host(), &fs, "n")
        .unwrap();
    let script = script(&prepared.plan);
    assert!(script.contains(&format!("& '{GIT}' 'clone'")));
    assert!(script.contains(&format!("'-v' '{FOLDER}:/workspace'")));
    assert_eq!(fs.created(), [ROOT]);
}

#[test]
fn a_failed_repository_start_creates_nothing_and_fails_in_german() {
    let dir = TempDir::new("service-repo-errors");
    let service = Service::open(&dir.0);
    let host = windows_host();
    let req = repo_request(AgentId::Claude, EnvId::Local, "traygent");
    let no_git = FakeFs::with(&[CLAUDE_EXE]);
    assert_eq!(
        service.prepare_launch(&req, &host, &no_git, "n"),
        Err("Git ist nicht installiert.".into())
    );
    assert!(no_git.created().is_empty());
    let invalid = repo_request(AgentId::Claude, EnvId::Local, "..");
    assert_eq!(
        service.prepare_launch(&invalid, &host, &no_git, "n"),
        Err(crate::repo::INVALID_NAME.into())
    );
    let mut bad_owner = req.clone();
    bad_owner.repo.as_mut().unwrap().owner = "a\\..\\b".into();
    assert_eq!(
        service.prepare_launch(&bad_owner, &host, &no_git, "n"),
        Err(crate::repo::INVALID_OWNER.into())
    );
    let reserved = repo_request(AgentId::Claude, EnvId::Local, "nul");
    assert!(service
        .prepare_launch(&reserved, &host, &no_git, "n")
        .unwrap_err()
        .contains("reservierten Gerätenamen"));
    let read_only = FakeFs {
        read_only: true,
        ..FakeFs::with(&[CLAUDE_EXE, GIT])
    };
    assert_eq!(
        service.prepare_launch(&req, &host, &read_only, "n"),
        Err("Der Klon-Ordner konnte nicht angelegt werden.".into())
    );
    let no_profile = Host {
        user_profile: None,
        ..host
    };
    let fs = FakeFs::with(&[CLAUDE_EXE, GIT]);
    let error = service
        .prepare_launch(&req, &no_profile, &fs, "n")
        .unwrap_err();
    assert!(error.starts_with("Kein Klon-Ordner verfügbar."), "{error}");
    assert!(fs.created().is_empty());
}

#[test]
fn a_configured_clone_root_wins_over_the_profile() {
    let dir = TempDir::new("service-repo-root");
    let mut service = Service::open(&dir.0);
    let mut settings = AppSettings::default();
    settings.github.clone_root = r" D:\src\ ".into();
    let saved = service.save_settings(settings).unwrap();
    assert_eq!(saved.snapshot.settings.github.clone_root, r"D:\src");
    assert!(!saved.start_detection);
    assert_eq!(
        Service::open(&dir.0).settings().github.clone_root,
        r"D:\src"
    );
    let fs = FakeFs::with(&[CLAUDE_EXE, GIT]);
    let req = repo_request(AgentId::Claude, EnvId::Local, "traygent");
    let prepared = service
        .prepare_launch(&req, &windows_host(), &fs, "n")
        .unwrap();
    assert_eq!(prepared.request.folder, r"D:\src\fo0\traygent");
    assert_eq!(fs.created(), [r"D:\src"]);
}

#[test]
fn an_invalid_clone_root_is_refused_on_save() {
    let dir = TempDir::new("service-repo-invalid-root");
    let mut service = Service::open(&dir.0);
    for root in ["relative", r"C:\a\..\b", r"\\?\C:\x", "C:\\a\u{1}"] {
        let mut settings = AppSettings::default();
        settings.github.clone_root = root.into();
        let error = service.save_settings(settings).unwrap_err();
        assert!(error.starts_with("Klon-Ordner: "), "{root}: {error}");
    }
    let mut settings = AppSettings::default();
    settings.github.clone_root = "  ".into();
    let saved = service.save_settings(settings).unwrap();
    assert_eq!(saved.snapshot.settings.github.clone_root, "");
}

#[test]
fn a_repository_start_records_the_folder_and_the_repo() {
    let dir = TempDir::new("service-repo-recent");
    let mut service = Service::open(&dir.0);
    let fs = FakeFs::with(&[CLAUDE_EXE]).dir(FOLDER);
    let req = repo_request(AgentId::Claude, EnvId::Local, "traygent.git");
    let prepared = service
        .prepare_launch(&req, &windows_host(), &fs, "n")
        .unwrap();
    let recent = service.record_launch(&prepared.request, Utc::now());
    assert_eq!(recent.folder, FOLDER);
    let repo = RepoRef {
        owner: "fo0".into(),
        name: "traygent".into(),
    };
    assert_eq!(recent.repo.as_ref(), Some(&repo));
    // One recent per folder: a start by path is the same entry.
    let by_path = request(AgentId::Claude, EnvId::Local, &FOLDER.to_lowercase());
    assert_eq!(
        recents::recent_id(&by_path.folder, by_path.agent, by_path.env),
        recent.id
    );
    let reloaded = Service::open(&dir.0).snapshot().recents;
    assert_eq!(reloaded, vec![recent.clone()]);
    service.remove_recent(&recent.id).unwrap();
    let mut invalid = recent.clone();
    invalid.repo.as_mut().unwrap().owner = "-x".into();
    assert_eq!(
        service.restore_recent(invalid),
        Err(crate::repo::INVALID_OWNER.into())
    );
    let mut suffixed = recent.clone();
    suffixed.repo.as_mut().unwrap().name = "traygent.git".into();
    let restored = service.restore_recent(suffixed).unwrap();
    assert_eq!(restored.recents[0].repo.as_ref(), Some(&repo));
}

#[test]
fn the_snapshot_carries_the_github_state_with_the_host_clone_root() {
    let dir = TempDir::new("service-repo-github");
    let mut service = Service::open(&dir.0);
    let github = service.snapshot().github;
    assert_eq!(github, GithubState::default());
    let before = service.snapshot().revision;
    service.set_default_clone_root(crate::repo::default_clone_root(&windows_host()));
    assert_eq!(service.snapshot().github.default_clone_root, ROOT);
    let account = GithubAccount {
        login: "max".into(),
        id: 7,
        source: GithubSource::Gh,
    };
    service.connect_github(
        account.clone(),
        &crate::github::vault::MemoryVault::default(),
    );
    let snapshot = service.snapshot();
    assert!(snapshot.revision > before);
    assert_eq!(snapshot.github.account, Some(account));
    assert!(snapshot.github.loading);
    assert_eq!(snapshot.github.default_clone_root, ROOT);
    assert_eq!(service.github(), &snapshot.github);
}
