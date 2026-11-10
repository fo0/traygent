//! The JSON shape is the IPC contract with `src/lib/types.ts`.
use super::*;
use serde_json::json;

fn text<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn enums_serialize_to_the_exact_ts_unions() {
    let agents: Vec<_> = AgentId::ALL.into_iter().map(text).collect();
    assert_eq!(agents, ["claude", "opencode", "codex"]);
    let envs: Vec<_> = EnvId::ALL.into_iter().map(text).collect();
    assert_eq!(envs, ["local", "docker", "wsl"]);
    let permissions = [PermissionId::Ask, PermissionId::Auto, PermissionId::Bypass].map(text);
    assert_eq!(permissions, ["ask", "auto", "bypass"]);
    let sessions = [SessionMode::New, SessionMode::Continue, SessionMode::Resume].map(text);
    assert_eq!(sessions, ["new", "continue", "resume"]);
    let terminals = [TerminalId::WindowsTerminal, TerminalId::Console].map(text);
    assert_eq!(terminals, ["windowsTerminal", "console"]);
    let states = [
        ProbeState::Ready,
        ProbeState::Missing,
        ProbeState::Stopped,
        ProbeState::Unavailable,
        ProbeState::Unknown,
    ]
    .map(text);
    assert_eq!(
        states,
        ["ready", "missing", "stopped", "unavailable", "unknown"]
    );
}

#[test]
fn ids_and_labels_match_the_serialized_form() {
    for agent in AgentId::ALL {
        assert_eq!(text(agent), agent.id());
    }
    for env in EnvId::ALL {
        assert_eq!(text(env), env.id());
    }
    let labels = AgentId::ALL.map(AgentId::label);
    assert_eq!(labels, ["Claude Code", "OpenCode", "Codex"]);
}

#[test]
fn unknown_enum_values_are_rejected() {
    assert!(serde_json::from_value::<AgentId>(json!("gemini")).is_err());
    assert!(serde_json::from_value::<TerminalId>(json!("windows_terminal")).is_err());
}

#[test]
fn launch_request_reads_the_panel_shape() {
    let request: LaunchRequest = serde_json::from_value(json!({
        "agent": "opencode",
        "env": "docker",
        "folder": "C:\\Code\\demo",
        "options": { "model": "opencode/big-pickle", "effort": "high", "permission": "bypass", "extraArgs": "--x" },
        "session": "continue"
    }))
    .unwrap();
    assert_eq!(request.agent, AgentId::Opencode);
    assert_eq!(request.env, EnvId::Docker);
    assert_eq!(request.options.extra_args, "--x");
    assert_eq!(request.options.permission, PermissionId::Bypass);
    assert_eq!(request.session, SessionMode::Continue);
    assert!(
        request.ports.is_empty(),
        "a request without ports publishes none"
    );
    let with_ports: LaunchRequest = serde_json::from_value(json!({
        "agent": "claude", "env": "docker", "folder": "C:\\x", "options": {},
        "session": "new", "ports": ["127.0.0.1:3000:3000"]
    }))
    .unwrap();
    assert_eq!(with_ports.ports, ["127.0.0.1:3000:3000"]);
}

#[test]
fn ports_default_to_empty_lists_in_old_recents_and_settings() {
    let recent: RecentSession = serde_json::from_value(json!({
        "id": "x", "agent": "claude", "env": "docker", "folder": "C:\\a",
        "options": {}, "lastStartedAt": "2026-11-03T09:00:00Z", "starts": 1
    }))
    .unwrap();
    assert!(recent.ports.is_empty());
    let docker: DockerSettings = serde_json::from_value(json!({ "ghToken": true })).unwrap();
    assert!(docker.ports.is_empty());
    assert_eq!(serde_json::to_value(&docker).unwrap()["ports"], json!([]));
    let preset = PortPreset {
        port: 3000,
        label: "Next.js · Node".into(),
    };
    assert_eq!(
        serde_json::to_value(&preset).unwrap(),
        json!({ "port": 3000, "label": "Next.js · Node" })
    );
}

#[test]
fn optional_fields_are_skipped_when_absent() {
    let status = AgentStatus {
        agent: AgentId::Codex,
        env: EnvId::Local,
        state: ProbeState::Missing,
        version: None,
        path: None,
        detail: None,
    };
    assert_eq!(
        serde_json::to_value(&status).unwrap(),
        json!({ "agent": "codex", "env": "local", "state": "missing" })
    );
    let choice = Choice {
        id: "default".into(),
        label: "Standard".into(),
        hint: None,
    };
    assert_eq!(
        serde_json::to_value(&choice).unwrap(),
        json!({ "id": "default", "label": "Standard" })
    );
}

#[test]
fn struct_fields_are_camel_case() {
    let recent = RecentSession {
        id: "abc".into(),
        agent: AgentId::Claude,
        env: EnvId::Local,
        folder: "C:\\x".into(),
        repo: Some(RepoRef {
            owner: "fo0".into(),
            name: "traygent".into(),
        }),
        options: LaunchOptions::default(),
        ports: vec!["127.0.0.1:3000:3000".into()],
        last_started_at: "2026-11-03T09:00:00Z".into(),
        starts: 2,
    };
    let value = serde_json::to_value(&recent).unwrap();
    assert_eq!(value["lastStartedAt"], "2026-11-03T09:00:00Z");
    assert_eq!(value["options"]["extraArgs"], "");
    assert_eq!(
        serde_json::from_value::<RecentSession>(value).unwrap(),
        recent
    );
    let install = InstallInfo {
        command: "x".into(),
        docs_url: "https://example.invalid".into(),
        update: None,
        requires: None,
    };
    assert!(serde_json::to_value(&install)
        .unwrap()
        .get("docsUrl")
        .is_some());
}

#[test]
fn per_agent_maps_serialize_as_records_and_fill_missing_keys() {
    let map = PerAgent::from_fn(|agent| agent.id().len());
    assert_eq!(
        serde_json::to_value(&map).unwrap(),
        json!({ "claude": 6, "opencode": 8, "codex": 5 })
    );
    let partial: PerAgent<String> = serde_json::from_value(json!({ "codex": "x" })).unwrap();
    assert_eq!(partial.get(AgentId::Codex), "x");
    assert_eq!(partial.get(AgentId::Claude), "");
    let mut map = map;
    *map.get_mut(AgentId::Opencode) = 1;
    assert_eq!(*map.get(AgentId::Opencode), 1);
}

#[test]
fn launch_options_default_to_no_flags_and_auto() {
    let options = LaunchOptions::default();
    assert_eq!(options.model, "default");
    assert_eq!(options.effort, "default");
    assert_eq!(options.permission, PermissionId::Auto);
    assert_eq!(options.extra_args, "");
    let partial: LaunchOptions = serde_json::from_value(json!({ "model": "opus" })).unwrap();
    assert_eq!(partial.effort, "default");
}

#[test]
fn repository_fields_are_optional_and_camel_case() {
    let request: LaunchRequest = serde_json::from_value(json!({
        "agent": "claude", "env": "local", "folder": "C:\\x",
        "options": {}, "session": "new", "ports": []
    }))
    .unwrap();
    assert_eq!(request.repo, None);
    assert!(serde_json::to_value(&request)
        .unwrap()
        .get("repo")
        .is_none());
    let request: LaunchRequest = serde_json::from_value(json!({
        "agent": "claude", "env": "local", "folder": "",
        "repo": { "owner": "fo0", "name": "traygent" },
        "options": {}, "session": "new", "ports": []
    }))
    .unwrap();
    assert_eq!(request.repo.unwrap().name, "traygent");
    let settings: AppSettings = serde_json::from_value(json!({})).unwrap();
    assert_eq!(settings.github.clone_root, "");
    assert!(!settings.shared_window);
    assert_eq!(
        serde_json::to_value(&settings).unwrap()["sharedWindow"],
        false
    );
    let shared: AppSettings = serde_json::from_value(json!({ "sharedWindow": true })).unwrap();
    assert!(shared.shared_window);
    // Settings from before the agent updates (#16) load with them off.
    assert!(!settings.agent_updates && !shared.agent_updates);
    assert_eq!(
        serde_json::to_value(&settings).unwrap()["agentUpdates"],
        false
    );
    let updating: AppSettings = serde_json::from_value(json!({ "agentUpdates": true })).unwrap();
    assert!(updating.agent_updates);
    assert_eq!(
        serde_json::to_value(&settings).unwrap()["github"],
        json!({ "cloneRoot": "" })
    );
}

#[test]
fn github_types_mirror_the_ts_contract() {
    assert_eq!(
        [GithubSource::Token, GithubSource::Gh].map(text),
        ["token", "gh"]
    );
    let empty = GithubState {
        default_clone_root: "C:\\Users\\max\\Traygent".into(),
        ..GithubState::default()
    };
    assert_eq!(
        serde_json::to_value(&empty).unwrap(),
        json!({
            "repos": [], "loading": false,
            "defaultCloneRoot": "C:\\Users\\max\\Traygent"
        })
    );
    let full = GithubState {
        account: Some(GithubAccount {
            login: "max".into(),
            id: 42,
            source: GithubSource::Gh,
        }),
        repos: vec![RepoSummary {
            owner: "fo0".into(),
            name: "traygent".into(),
            private: true,
            pushed_at: Some("2026-11-03T09:00:00Z".into()),
            description: None,
        }],
        loading: true,
        error: Some("GitHub-Token ungültig oder abgelaufen.".into()),
        listed_at: Some("2026-11-03T09:05:00Z".into()),
        default_clone_root: String::new(),
    };
    let value = serde_json::to_value(&full).unwrap();
    assert_eq!(
        value["account"],
        json!({ "login": "max", "id": 42, "source": "gh" })
    );
    assert_eq!(
        value["repos"][0],
        json!({
            "owner": "fo0", "name": "traygent", "private": true,
            "pushedAt": "2026-11-03T09:00:00Z"
        })
    );
    assert_eq!(value["listedAt"], "2026-11-03T09:05:00Z");
    assert_eq!(serde_json::from_value::<GithubState>(value).unwrap(), full);
    let accounts = LocalAccounts {
        accounts: vec![LocalAccount {
            login: "max".into(),
            importable: false,
            connected: true,
        }],
        message: None,
    };
    assert_eq!(
        serde_json::to_value(&accounts).unwrap(),
        json!({ "accounts": [{ "login": "max", "importable": false, "connected": true }] })
    );
}

#[test]
fn agent_update_statuses_are_camel_case_and_skip_unknown_versions() {
    let states = [
        (AgentUpdateState::Running, "running"),
        (AgentUpdateState::Updated, "updated"),
        (AgentUpdateState::Current, "current"),
        (AgentUpdateState::InUse, "inUse"),
        (AgentUpdateState::Failed, "failed"),
    ];
    for (state, text) in states {
        assert_eq!(serde_json::to_value(state).unwrap(), json!(text));
        let back: AgentUpdateState = serde_json::from_value(json!(text)).unwrap();
        assert_eq!(back, state);
    }
    let skipped = AgentUpdateStatus {
        agent: AgentId::Codex,
        state: AgentUpdateState::InUse,
        version: None,
        from: None,
        at: "2026-11-10T08:00:00Z".into(),
    };
    assert_eq!(
        serde_json::to_value(&skipped).unwrap(),
        json!({ "agent": "codex", "state": "inUse", "at": "2026-11-10T08:00:00Z" })
    );
    let updated = AgentUpdateStatus {
        state: AgentUpdateState::Updated,
        version: Some("0.167.0".into()),
        from: Some("0.166.0".into()),
        ..skipped
    };
    let value = serde_json::to_value(&updated).unwrap();
    assert_eq!(
        (&value["version"], &value["from"]),
        (&json!("0.167.0"), &json!("0.166.0"))
    );
    assert_eq!(
        serde_json::from_value::<AgentUpdateStatus>(value).unwrap(),
        updated
    );
    // A snapshot always carries the list; one from before it reads as empty.
    let mut native = serde_json::to_value(Snapshot::native()).unwrap();
    assert_eq!(native["agentUpdates"], json!([]));
    native.as_object_mut().unwrap().remove("agentUpdates");
    let old: Snapshot = serde_json::from_value(native).unwrap();
    assert!(old.agent_updates.is_empty());
}
