use super::*;
use crate::settings;

fn with(dirs: &[&str], files: &[&str]) -> AppSettings {
    let mut settings = AppSettings::default();
    settings.profile_share.claude = share(dirs, files);
    settings
}

fn share(dirs: &[&str], files: &[&str]) -> ProfileShare {
    let owned = |list: &[&str]| list.iter().map(|name| name.to_string()).collect();
    ProfileShare {
        dirs: owned(dirs),
        files: owned(files),
    }
}

fn error(dirs: &[&str], files: &[&str]) -> String {
    settings::validate(&settings::normalize(with(dirs, files))).unwrap_err()
}

#[test]
fn nothing_extra_is_shared_by_default_and_names_are_trimmed() {
    let defaults = AppSettings::default();
    for agent in AgentId::ALL {
        assert_eq!(defaults.profile_share.get(agent), &ProfileShare::default());
    }
    let raw = with(
        &[" .caveman-sessions ", "", "notes", "notes", "  "],
        &["orca-state.json", " .caveman-active", "orca-state.json"],
    );
    let settings = settings::normalize(raw);
    assert_eq!(settings::validate(&settings), Ok(()));
    assert_eq!(
        settings.profile_share.claude,
        share(
            &[".caveman-sessions", "notes"],
            &["orca-state.json", ".caveman-active"]
        )
    );
    let json = serde_json::to_value(&settings).unwrap();
    assert_eq!(
        json["profileShare"]["claude"]["dirs"],
        serde_json::json!([".caveman-sessions", "notes"])
    );
    assert_eq!(
        json["profileShare"]["codex"],
        serde_json::json!({"dirs": [], "files": []})
    );
}

#[test]
fn every_refused_name_has_its_error() {
    let invalid = |name: &str| format!("Name „{name}“ für Claude Code ist ungültig.");
    let long = "a".repeat(65);
    for name in [
        ".",
        "..",
        "...",
        "a.",
        "a b",
        "a/b",
        "a\\b",
        "..\\x",
        "C:",
        "ä",
        "a*",
        "a\"b",
        "%TEMP%",
        "NUL",
        "nul",
        "con.txt",
        "COM1",
        "lpt9.log",
        "Aux",
        "prn",
        ".traygent-sync.json",
        ".traygent-base",
        ".Traygent-backup",
        ".traygentx",
        &long,
    ] {
        assert_eq!(error(&[name], &[]), invalid(name), "dir {name}");
        assert_eq!(error(&[], &[name]), invalid(name), "file {name}");
    }
    let handled = |name: &str| format!("Name „{name}“ für Claude Code wird bereits abgeglichen.");
    for name in [
        "skills",
        "Projects",
        "hooks",
        "CLAUDE.md",
        "claude.md",
        "keybindings.json",
        "settings.json",
        "history.jsonl",
        "todos",
        "session-env",
        "History.JSONL",
    ] {
        assert_eq!(error(&[name], &[]), handled(name), "dir {name}");
        assert_eq!(error(&[], &[name]), handled(name), "file {name}");
    }
    // Unique across both lists, whatever the case.
    let twice = "Name „Notes“ für Claude Code ist doppelt vergeben.";
    assert_eq!(error(&["notes", "Notes"], &[]), twice);
    assert_eq!(error(&["notes"], &["Notes"]), twice);
    assert_eq!(error(&[], &["notes", "x", "Notes"]), twice);
    // At most 16 each.
    let many: Vec<String> = (0..17).map(|index| format!("n{index}")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    assert_eq!(
        error(&many, &[]),
        "Höchstens 16 weitere Ordner für Claude Code möglich."
    );
    assert_eq!(
        error(&[], &many),
        "Höchstens 16 weitere Dateien für Claude Code möglich."
    );
    assert_eq!(settings::validate(&with(&many[..16], &["f"])), Ok(()));
    // Fine: dots, dashes, underscores, 64 characters, a device name only as a part.
    let longest = "a".repeat(64);
    let fine = with(
        &[".caveman-sessions", "a_b-c.d", &longest, "console", "com10"],
        &["nul-x"],
    );
    assert_eq!(settings::validate(&fine), Ok(()));
    // An agent without profiles shares nothing.
    for agent in [AgentId::Opencode, AgentId::Codex] {
        let mut settings = AppSettings::default();
        settings.profile_share.get_mut(agent).files = vec!["notes".into()];
        assert_eq!(
            settings::validate(&settings),
            Err(format!("{} unterstützt keine Profile.", agent.label()))
        );
    }
}

#[test]
fn loaded_settings_keep_only_the_names_that_validate() {
    let mut loaded = with(
        &[
            "notes",
            "..",
            "skills",
            "NOTES",
            " spaced ",
            ".credentials.json",
            "a/b",
        ],
        &[
            "notes",
            "orca-state.json",
            "nul",
            ".traygent-sync.json",
            "settings.json",
        ],
    );
    loaded.profile_share.codex = share(&["notes"], &["x"]);
    let clean = settings::sanitize(loaded);
    assert_eq!(
        clean.profile_share.claude,
        share(&["notes", "spaced"], &["orca-state.json"])
    );
    assert_eq!(clean.profile_share.codex, ProfileShare::default());
    assert_eq!(settings::validate(&clean), Ok(()));
    // More than the cap: the first 16 stay.
    let many: Vec<String> = (0..20).map(|index| format!("n{index}")).collect();
    let mut loaded = AppSettings::default();
    loaded.profile_share.claude.files = many.clone();
    assert_eq!(
        settings::sanitize(loaded).profile_share.claude.files,
        many[..16]
    );
}
