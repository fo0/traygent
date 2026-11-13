//! The extra shared names in `state.json` (#35): old files, the round trip, a tampered file.
use super::*;
use crate::models::ProfileShare;

fn share(dirs: &[&str], files: &[&str]) -> ProfileShare {
    let owned = |list: &[&str]| list.iter().map(|name| name.to_string()).collect();
    ProfileShare {
        dirs: owned(dirs),
        files: owned(files),
    }
}

#[test]
fn a_file_without_the_setting_shares_nothing_extra() {
    let dir = TempDir::new("share-old");
    dir.write(&json!({"version": 1, "settings": {"terminal": "console"}}).to_string());
    let loaded = load(&dir.0);
    assert!(loaded.read_only.is_none() && loaded.backup.is_none());
    assert_eq!(loaded.state.settings.terminal, TerminalId::Console);
    for agent in AgentId::ALL {
        assert_eq!(
            loaded.state.settings.profile_share.get(agent),
            &ProfileShare::default()
        );
    }
}

#[test]
fn the_extra_names_survive_a_save_and_a_load() {
    let dir = TempDir::new("share-round-trip");
    let mut state = sample();
    state.settings.profile_share.claude =
        share(&[".caveman-sessions", "notes"], &["orca-state.json"]);
    save(&dir.0, &state).unwrap();
    let text = std::fs::read_to_string(path(&dir.0)).unwrap();
    let stored: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        stored["settings"]["profileShare"]["claude"],
        json!({"dirs": [".caveman-sessions", "notes"], "files": ["orca-state.json"]})
    );
    assert_eq!(load(&dir.0).state, state);
}

/// Nothing unchecked from a hand-edited file reaches the sync: every name that would not
/// validate is dropped on load, the rest kept.
#[test]
fn a_tampered_file_loads_without_its_invalid_names() {
    let dir = TempDir::new("share-tampered");
    dir.write(
        &json!({
            "version": 1,
            "settings": {
                "profileShare": {
                    "claude": {
                        "dirs": [
                            "notes", "..", "..\\..\\Windows", "C:\\Users", 7, null, "skills",
                            ".credentials.json", ".traygent-backup", "NUL", " kept "
                        ],
                        "files": ["Notes", "orca-state.json", ".claude.json", {"x": 1}, "a/b"]
                    },
                    "codex": {"dirs": ["notes"], "files": ["x"]},
                    "opencode": "everything"
                }
            }
        })
        .to_string(),
    );
    let state = load(&dir.0).state;
    assert_eq!(
        state.settings.profile_share.claude,
        share(&["notes", "kept"], &["orca-state.json"])
    );
    assert_eq!(state.settings.profile_share.codex, ProfileShare::default());
    assert_eq!(
        state.settings.profile_share.opencode,
        ProfileShare::default()
    );
    assert_eq!(crate::settings::validate(&state.settings), Ok(()));
    // Lists of the wrong shape load as empty.
    dir.write(
        &json!({"version": 1, "settings": {"profileShare": {"claude": {"dirs": "notes", "files": 3}}}})
            .to_string(),
    );
    assert_eq!(
        load(&dir.0).state.settings.profile_share.claude,
        ProfileShare::default()
    );
}
