//! Profiles in `state.json` (#33): old files, lenient loading, the round trip.
use super::*;

/// A file from before the profiles (#33) loads without a version bump: no extra profile,
/// the default one chosen, recents without a profile.
#[test]
fn a_file_without_profiles_loads_with_their_defaults() {
    let dir = TempDir::new("profiles-old");
    dir.write(
        &json!({
            "version": 1,
            "settings": { "terminal": "console", "agentUpdates": true },
            "recents": [{
                "id": "x", "agent": "claude", "env": "local", "folder": "C:\\Code\\demo",
                "options": {}, "lastStartedAt": "2026-11-03T09:00:00Z", "starts": 1
            }]
        })
        .to_string(),
    );
    let loaded = load(&dir.0);
    assert!(loaded.read_only.is_none() && loaded.backup.is_none());
    let settings = &loaded.state.settings;
    assert_eq!(settings.terminal, TerminalId::Console);
    assert!(settings.agent_updates);
    assert!(
        !settings.debug,
        "a file from before „Debug“ loads with it off"
    );
    for agent in AgentId::ALL {
        assert!(settings.profiles.get(agent).is_empty());
        assert_eq!(settings.agent_profile.get(agent), "default");
    }
    assert_eq!(loaded.state.recents[0].profile, None);
}

#[test]
fn profiles_and_recent_profiles_load_leniently() {
    let dir = TempDir::new("profiles-lenient");
    let recent = |folder: &str, env: &str, profile: Value| {
        json!({
            "id": folder, "agent": "claude", "env": env, "folder": folder, "options": {},
            "profile": profile, "lastStartedAt": "2026-11-03T09:00:00Z", "starts": 1
        })
    };
    dir.write(
        &json!({
            "version": 1,
            "settings": {
                "profiles": {
                    "claude": [
                        { "id": "team", "label": "Team" },
                        "nonsense",
                        { "id": "..\\evil", "label": "Böse" },
                        { "id": "solo" },
                        { "id": "max", "label": "Max", "later": true }
                    ],
                    "codex": [{ "id": "team", "label": "Team" }],
                    "opencode": "none"
                },
                "agentProfile": { "claude": "max", "codex": "team", "opencode": 7 }
            },
            "recents": [
                recent("C:\\a", "local", json!("team")),
                recent("C:\\b", "local", json!("..\\..\\x")),
                recent("C:\\c", "docker", json!("team")),
                recent("C:\\d", "local", json!("default")),
                recent("C:\\e", "local", json!("gone"))
            ]
        })
        .to_string(),
    );
    let state = load(&dir.0).state;
    let ids: Vec<_> = state
        .settings
        .profiles
        .claude
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(ids, ["team", "max"]);
    assert!(state.settings.profiles.codex.is_empty());
    assert!(state.settings.profiles.opencode.is_empty());
    assert_eq!(state.settings.agent_profile.claude, "max");
    assert_eq!(state.settings.agent_profile.codex, "default");
    assert_eq!(state.settings.agent_profile.opencode, "default");
    let by_folder = |folder: &str| {
        let recent = state.recents.iter().find(|recent| recent.folder == folder);
        recent.unwrap().profile.clone()
    };
    assert_eq!(by_folder("C:\\a").as_deref(), Some("team"));
    assert_eq!(by_folder("C:\\b"), None, "a malformed id");
    assert_eq!(by_folder("C:\\c"), None, "a container has its own home");
    assert_eq!(
        by_folder("C:\\d"),
        None,
        "the default profile is stored as none"
    );
    // A removed profile stays on the recent; its start says „Profil nicht gefunden.“.
    assert_eq!(by_folder("C:\\e").as_deref(), Some("gone"));
    // What loaded saves and loads again unchanged.
    save(&dir.0, &state).unwrap();
    assert_eq!(load(&dir.0).state, state);
}
