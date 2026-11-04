use super::*;
use crate::models::{
    EnvId, GithubSource, LaunchRequest, PermissionId, RepoRef, SessionMode, TerminalId,
};
use crate::testing::TempDir;
use serde_json::json;

trait WriteState {
    fn write(&self, text: &str);
}

impl WriteState for TempDir {
    fn write(&self, text: &str) {
        std::fs::write(path(&self.0), text).unwrap();
    }
}

fn sample() -> Persisted {
    let mut settings = AppSettings {
        terminal: TerminalId::Console,
        shared_window: true,
        ..AppSettings::default()
    };
    settings.docker.gh_token = true;
    settings.docker.ports = vec!["127.0.0.1:5173:5173".into()];
    settings.agents.get_mut(AgentId::Codex).permission = PermissionId::Bypass;
    settings.github.clone_root = r"D:\src".into();
    let mut recents = Vec::new();
    let request = LaunchRequest {
        agent: AgentId::Claude,
        env: EnvId::Docker,
        folder: "C:\\Code\\demo".into(),
        repo: Some(RepoRef {
            owner: "fo0".into(),
            name: "demo".into(),
        }),
        options: Default::default(),
        session: SessionMode::Continue,
        ports: vec!["3000".into(), "0.0.0.0:8080:80/udp".into()],
    };
    recents::upsert(&mut recents, &request, chrono::Utc::now());
    let github = Some(GithubAccount {
        login: "octo-cat".into(),
        id: 42,
        source: GithubSource::Gh,
    });
    Persisted {
        settings,
        recents,
        github,
    }
}

#[test]
fn a_missing_file_yields_writable_defaults() {
    let dir = TempDir::new("missing");
    let loaded = load(&dir.0);
    assert_eq!(loaded.state, Persisted::default());
    assert!(loaded.read_only.is_none());
    assert!(loaded.backup.is_none());
}

#[test]
fn save_then_load_round_trips_with_a_version() {
    let dir = TempDir::new("roundtrip");
    let state = sample();
    save(&dir.0, &state).unwrap();
    let text = std::fs::read_to_string(path(&dir.0)).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["version"], 1);
    assert_eq!(
        value["settings"]["github"],
        json!({ "cloneRoot": "D:\\src" })
    );
    assert_eq!(
        value["recents"][0]["repo"],
        json!({ "owner": "fo0", "name": "demo" })
    );
    assert_eq!(value["settings"]["terminal"], "console");
    assert_eq!(value["settings"]["sharedWindow"], true);
    assert_eq!(
        value["github"],
        json!({ "login": "octo-cat", "id": 42, "source": "gh" })
    );
    assert!(text.ends_with("}\n"));
    // The temp file is renamed over the target, nothing else stays behind.
    assert_eq!(dir.entries(), [FILE]);
    let loaded = load(&dir.0);
    assert_eq!(loaded.state, state);
    assert!(loaded.read_only.is_none());
}

#[test]
fn save_creates_the_directory_and_replaces_an_existing_file() {
    let dir = TempDir::new("create");
    let nested = dir.0.join("app").join("data");
    save(&nested, &Persisted::default()).unwrap();
    let state = sample();
    save(&nested, &state).unwrap();
    assert_eq!(load(&nested).state, state);
}

#[test]
fn a_corrupt_file_is_backed_up_and_replaced_by_defaults() {
    for (name, text) in [
        ("syntax", "{ not json"),
        ("array", "[1, 2]"),
        ("noversion", r#"{"settings":{}}"#),
        ("zero", r#"{"version":0}"#),
        ("text", r#"{"version":"1"}"#),
    ] {
        let dir = TempDir::new(name);
        dir.write(text);
        let loaded = load(&dir.0);
        assert_eq!(loaded.state, Persisted::default(), "{name}");
        assert!(loaded.read_only.is_none(), "{name}");
        let backup = loaded.backup.expect("backup");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), text);
        let names = dir.entries();
        assert_eq!(names.len(), 1, "{name}");
        assert!(names[0].starts_with("state.json.corrupt-"), "{name}");
        // The next save writes a fresh state.json next to the backup.
        save(&dir.0, &Persisted::default()).unwrap();
        assert_eq!(dir.entries().len(), 2);
    }
}

#[test]
fn two_corrupt_files_in_the_same_second_get_distinct_backups() {
    let dir = TempDir::new("twice");
    dir.write("broken");
    let first = load(&dir.0).backup.unwrap();
    dir.write("broken again");
    let second = load(&dir.0).backup.unwrap();
    assert_ne!(first, second);
    assert_eq!(dir.entries().len(), 2);
}

#[test]
fn a_newer_version_is_read_but_refuses_writes() {
    let dir = TempDir::new("newer");
    let text = json!({
        "version": 2,
        "settings": { "terminal": "console", "futureField": true },
        "recents": []
    })
    .to_string();
    dir.write(&text);
    let loaded = load(&dir.0);
    assert_eq!(loaded.read_only.as_deref(), Some(NEWER_VERSION));
    assert_eq!(loaded.state.settings.terminal, TerminalId::Console);
    assert!(loaded.backup.is_none());
    assert_eq!(std::fs::read_to_string(path(&dir.0)).unwrap(), text);
}

#[test]
fn unknown_or_invalid_fields_fall_back_per_field() {
    let dir = TempDir::new("lenient");
    dir.write(
        &json!({
            "version": 1,
            "extra": "ignored",
            "settings": {
                "defaultAgent": "gemini",
                "defaultEnv": "docker",
                "terminal": "alacritty",
                "sharedWindow": "yes",
                "agents": {
                    "claude": { "model": "opus", "permission": "bypass" },
                    "codex": { "permission": "sometimes" }
                },
                "docker": { "images": { "codex": "ghcr.io/x/codex:1", "claude": 42 }, "ghToken": true }
            },
            "recents": [
                { "id": "x", "agent": "claude", "env": "local", "folder": "C:\\a",
                  "options": {}, "lastStartedAt": "2026-11-03T09:00:00Z", "starts": 3 },
                { "id": "y", "agent": "gemini", "env": "local", "folder": "C:\\b",
                  "options": {}, "lastStartedAt": "2026-11-03T09:00:00Z", "starts": 1 },
                "garbage"
            ]
        })
        .to_string(),
    );
    let loaded = load(&dir.0);
    assert!(loaded.read_only.is_none());
    let settings = &loaded.state.settings;
    assert_eq!(settings.default_agent, AgentId::Claude);
    assert_eq!(settings.default_env, EnvId::Docker);
    assert_eq!(settings.terminal, TerminalId::WindowsTerminal);
    assert!(!settings.shared_window);
    let claude = settings.agents.get(AgentId::Claude);
    assert_eq!(
        (claude.model.as_str(), claude.permission),
        ("opus", PermissionId::Bypass)
    );
    assert_eq!(claude.effort, "default");
    assert_eq!(
        settings.agents.get(AgentId::Codex),
        &LaunchOptions::default()
    );
    assert_eq!(
        settings.agents.get(AgentId::Opencode),
        &LaunchOptions::default()
    );
    assert_eq!(
        settings.docker.images.get(AgentId::Codex),
        "ghcr.io/x/codex:1"
    );
    assert_eq!(
        settings.docker.images.get(AgentId::Claude),
        "node:22-trixie"
    );
    assert_eq!(
        settings.docker.images.get(AgentId::Opencode),
        "node:22-trixie"
    );
    assert!(settings.docker.gh_token);
    assert_eq!(loaded.state.recents.len(), 1);
    assert_eq!(loaded.state.recents[0].starts, 3);
}

/// A file from before the setting offers every environment; a newer id is dropped alone.
#[test]
fn environments_load_with_their_default_and_drop_unknown_ids() {
    let dir = TempDir::new("environments");
    dir.write(r#"{"version":1,"settings":{"defaultEnv":"docker"}}"#);
    let settings = load(&dir.0).state.settings;
    assert_eq!(settings.environments, EnvId::ALL.to_vec());
    assert_eq!(settings.default_env, EnvId::Docker);
    // A file from before the shared window setting opens a new window per start.
    assert!(!settings.shared_window);
    dir.write(
        r#"{"version":1,"settings":{"environments":["ssh","docker","wsl","docker"],"defaultEnv":"local"}}"#,
    );
    let settings = load(&dir.0).state.settings;
    assert_eq!(settings.environments, [EnvId::Docker, EnvId::Wsl]);
    assert_eq!(settings.default_env, EnvId::Docker);
}

#[cfg(unix)]
#[test]
fn an_unreadable_file_is_never_overwritten() {
    let dir = TempDir::new("unreadable");
    // A directory named state.json cannot be read as a file.
    std::fs::create_dir(path(&dir.0)).unwrap();
    let loaded = load(&dir.0);
    assert!(loaded.read_only.is_some());
    assert!(path(&dir.0).is_dir());
}

#[test]
fn a_failed_save_reports_in_german_and_leaves_no_temp_file() {
    let dir = TempDir::new("blocked");
    std::fs::create_dir(path(&dir.0)).unwrap();
    std::fs::write(path(&dir.0).join("keep"), "x").unwrap();
    assert_eq!(
        save(&dir.0, &Persisted::default()),
        Err("Einstellungen konnten nicht gespeichert werden.".into())
    );
    assert_eq!(dir.entries(), [FILE]);
}

#[test]
fn files_without_ports_load_empty_lists_and_invalid_ports_reset_only_themselves() {
    let dir = TempDir::new("ports");
    let recent = |id: &str, env: &str, ports: serde_json::Value| {
        let mut entry = json!({ "id": id, "agent": "claude", "env": env, "folder": "C:\\a",
            "options": {}, "lastStartedAt": "2026-11-03T09:00:00Z", "starts": 1 });
        if !ports.is_null() {
            entry["ports"] = ports;
        }
        entry
    };
    dir.write(
        &json!({
            "version": 1,
            "settings": { "docker": { "ghToken": true } },
            "recents": [recent("old", "docker", serde_json::Value::Null)]
        })
        .to_string(),
    );
    let loaded = load(&dir.0);
    assert!(loaded.state.settings.docker.ports.is_empty());
    assert!(loaded.state.settings.docker.gh_token);
    assert_eq!(loaded.state.recents.len(), 1);
    assert!(loaded.state.recents[0].ports.is_empty());
    dir.write(
        &json!({
            "version": 1,
            "settings": { "docker": { "ghToken": true, "ports": ["3000", "3000 --privileged"] } },
            "recents": [
                recent("a", "docker", json!(["8080:80", "-p"])),
                recent("b", "docker", json!([" 3000", "0.0.0.0:53:53/udp"])),
                recent("c", "local", json!(["3000"])),
                recent("d", "docker", json!("3000"))
            ]
        })
        .to_string(),
    );
    let loaded = load(&dir.0);
    assert!(loaded.state.settings.docker.ports.is_empty());
    assert!(loaded.state.settings.docker.gh_token);
    let ports: Vec<(&str, Vec<String>)> = loaded
        .state
        .recents
        .iter()
        .map(|recent| (recent.id.as_str(), recent.ports.clone()))
        .collect();
    assert_eq!(
        ports,
        [
            ("a", vec![]),
            (
                "b",
                vec!["127.0.0.1:3000:3000".into(), "0.0.0.0:53:53/udp".into()]
            ),
            ("c", vec![])
        ]
    );
    dir.write(r#"{"version":1,"settings":{"docker":{"ports":[" 5173","8080:80/tcp"]}}}"#);
    assert_eq!(
        load(&dir.0).state.settings.docker.ports,
        ["127.0.0.1:5173:5173", "127.0.0.1:8080:80"]
    );
}

#[test]
fn the_clone_root_and_recent_repos_load_leniently() {
    let recent = |repo: Value| {
        json!({
            "id": "x", "agent": "claude", "env": "local", "folder": "C:\\a",
            "repo": repo, "options": {}, "ports": [],
            "lastStartedAt": "2026-11-03T09:00:00Z", "starts": 1
        })
    };
    for (clone_root, expected) in [
        (json!(r"D:\src\"), r"D:\src"),
        (json!("relative"), ""),
        (json!(5), ""),
        (json!(null), ""),
    ] {
        let dir = TempDir::new("github-lenient");
        let text = json!({
            "version": 1,
            "settings": { "terminal": "console", "github": { "cloneRoot": clone_root } },
            "recents": [recent(json!({ "owner": "-bad", "name": "x" }))]
        });
        dir.write(&text.to_string());
        let loaded = load(&dir.0).state;
        assert_eq!(loaded.settings.github.clone_root, expected, "{clone_root}");
        assert_eq!(loaded.settings.terminal, TerminalId::Console);
        // An invalid repository is dropped; the entry still starts by its folder.
        assert_eq!(loaded.recents.len(), 1);
        assert_eq!(loaded.recents[0].repo, None);
    }
    let dir = TempDir::new("github-missing");
    let text = json!({
        "version": 1,
        "settings": {},
        "recents": [recent(json!({ "owner": "fo0", "name": "x.git" }))]
    });
    dir.write(&text.to_string());
    let loaded = load(&dir.0).state;
    assert_eq!(loaded.settings.github.clone_root, "");
    let repo = loaded.recents[0].repo.as_ref().unwrap();
    assert_eq!((repo.owner.as_str(), repo.name.as_str()), ("fo0", "x"));
}

#[test]
fn the_github_account_loads_leniently_and_is_left_out_without_one() {
    let account = |login: &str, id: u64| GithubAccount {
        login: login.into(),
        id,
        source: GithubSource::Token,
    };
    for (github, expected) in [
        (
            json!({ "login": "octo-cat", "id": 42, "source": "token", "token": "x" }),
            Some(account("octo-cat", 42)),
        ),
        (
            json!({ "login": "octo-cat", "id": 42, "source": "oauth" }),
            None,
        ),
        (
            json!({ "login": "-bad", "id": 42, "source": "token" }),
            None,
        ),
        (
            json!({ "login": "octo-cat", "id": 0, "source": "token" }),
            None,
        ),
        (json!({ "login": "octo-cat", "source": "token" }), None),
        (json!("octo-cat"), None),
    ] {
        let dir = TempDir::new("github-account");
        let text = json!({ "version": 1, "settings": { "terminal": "console" }, "github": github });
        dir.write(&text.to_string());
        let loaded = load(&dir.0).state;
        assert_eq!(loaded.github, expected, "{github}");
        assert_eq!(loaded.settings.terminal, TerminalId::Console);
    }
    let dir = TempDir::new("github-none");
    save(&dir.0, &Persisted::default()).unwrap();
    let text = std::fs::read_to_string(path(&dir.0)).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert!(value.get("github").is_none());
}
