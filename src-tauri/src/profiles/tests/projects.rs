//! Step 3 of #35: the directories the owner's script shared, the state file's whole-value
//! keys and the runtime fields of a project entry, which stay with their profile.
use super::*;
use crate::profiles::json::Shared;
use crate::profiles::state::{merge_keys, Known};
use serde_json::{json, Value};

fn parsed(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap()
}

/// A state file with one project entry, written the way the CLI writes it.
fn state(account: &str, project: Value) -> String {
    let doc = json!({
        "oauthAccount": {"emailAddress": account},
        "projects": {"C:/Code/demo": project},
    });
    serde_json::to_string_pretty(&doc).unwrap()
}

/// The state file's bytes and file time.
fn stamp(path: &Path) -> (String, SystemTime) {
    (
        read(path),
        std::fs::metadata(path).unwrap().modified().unwrap(),
    )
}

#[test]
fn hooks_todos_and_the_session_environment_are_shared_directories() {
    let shared = recipe(CLAUDE).unwrap().shared_dirs;
    let world = World::new("profiles-more-dirs");
    for name in ["hooks", "todos", "session-env"] {
        assert!(shared.contains(&name), "{name}");
        write(&world.dir("team").join(name).join("own.txt"), name);
    }
    own(&world, "team");
    world.synced(&["team"], 1);
    for name in ["hooks", "todos", "session-env"] {
        // Adopted into the default home, then one directory under two names.
        assert_eq!(read(&world.dir("default").join(name).join("own.txt")), name);
        let link = world.dir("team").join(name);
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        write(&world.dir("default").join(name).join("new.txt"), "new");
        assert_eq!(read(&link.join("new.txt")), "new");
    }
}

#[test]
fn whole_value_keys_follow_the_winning_file() {
    let rule = recipe(CLAUDE).unwrap();
    assert_eq!(
        rule.shared_values,
        ["showSpinnerTree", "claudeInChromeDefaultEnabled"]
    );
    let shared = Shared::of(rule);
    let merge = |ours: &str, theirs: &str, default_wins: bool| {
        let merged = merge_keys(
            Some(ours.as_bytes()),
            Some(theirs.as_bytes()),
            &shared,
            // A base that knows nothing: the newer file wins.
            Known::Values(&[]),
            default_wins,
        )
        .unwrap();
        let value = |bytes: Option<Vec<u8>>| {
            bytes.map(|bytes| serde_json::from_slice::<Value>(&bytes).unwrap())
        };
        (value(merged.default), value(merged.profile))
    };
    // The winner's value replaces the other; equal values write nothing.
    let (ours, theirs) = (
        r#"{"showSpinnerTree":false}"#,
        r#"{"showSpinnerTree":true}"#,
    );
    assert_eq!(
        merge(ours, theirs, true),
        (None, Some(json!({"showSpinnerTree": false})))
    );
    assert_eq!(
        merge(ours, theirs, false),
        (Some(json!({"showSpinnerTree": true})), None)
    );
    assert_eq!(merge(ours, ours, false), (None, None));
    // A file without the key removes nothing: it gets the other side's, whoever wins.
    for default_wins in [true, false] {
        assert_eq!(
            merge(
                "{}",
                r#"{"claudeInChromeDefaultEnabled":true}"#,
                default_wins
            ),
            (Some(json!({"claudeInChromeDefaultEnabled": true})), None)
        );
    }

    // Through the sync: the default home on the first sync, later the newer file.
    let world = World::new("profiles-whole-values");
    write(&world.state("default"), r#"{"showSpinnerTree":false}"#);
    write(
        &world.state("team"),
        r#"{"showSpinnerTree":true,"claudeInChromeDefaultEnabled":true}"#,
    );
    touch(&world.state("default"), 10);
    touch(&world.state("team"), 500);
    world.synced(&["team"], 1);
    let both = json!({"showSpinnerTree": false, "claudeInChromeDefaultEnabled": true});
    assert_eq!(parsed(&world.state("default")), both);
    assert_eq!(parsed(&world.state("team")), both);
    write(
        &world.state("team"),
        r#"{"showSpinnerTree":true,"claudeInChromeDefaultEnabled":true}"#,
    );
    touch(&world.state("default"), 600);
    touch(&world.state("team"), 700);
    world.synced(&["team"], 2);
    assert_eq!(parsed(&world.state("default"))["showSpinnerTree"], true);
}

#[test]
fn project_entries_that_differ_only_in_runtime_fields_cause_no_write() {
    let own_fields = recipe(CLAUDE).unwrap().own_fields.as_ref().unwrap();
    for name in [
        "lastCost",
        "lastSessionId",
        "last",
        "hasUnseenTeamArtifacts",
    ] {
        assert!(own_fields.holds(name), "{name}");
    }
    for name in [
        "allowedTools",
        "hasUnseenTeamArtifactsToo",
        "blast",
        "atLast",
    ] {
        assert!(!own_fields.holds(name), "{name}");
    }
    let world = World::new("profiles-runtime-fields");
    let ours = state(
        "max@example.test",
        json!({
            "allowedTools": ["Read"], "lastCost": 1.25, "lastSessionId": "session-default",
            "hasUnseenTeamArtifacts": false
        }),
    );
    let theirs = state(
        "team@example.test",
        json!({
            "lastSessionId": "session-team", "allowedTools": ["Read"], "lastCost": 9.5,
            "lastDuration": 12, "hasUnseenTeamArtifacts": true
        }),
    );
    write(&world.state("default"), &ours);
    write(&world.state("team"), &theirs);
    touch(&world.state("default"), 10);
    touch(&world.state("team"), 500);
    let before = (stamp(&world.state("default")), stamp(&world.state("team")));
    // On the first sync (the default home wins) and later (the profile's file is newer).
    for minute in [1, 2] {
        world.synced(&["team"], minute);
        assert_eq!(stamp(&world.state("default")), before.0, "run {minute}");
        assert_eq!(stamp(&world.state("team")), before.1, "run {minute}");
    }
    assert!(!world.dir("team").join(".traygent-backup").exists());
    assert!(!world.dir("default").join(".traygent-backup").exists());
}

#[test]
fn a_changed_project_field_travels_and_each_side_keeps_its_runtime_fields() {
    let world = World::new("profiles-project-fields");
    let ours = r#"{
  "bigNumber": 12345678901234567890.5000,
  "oauthAccount": { "emailAddress": "max@example.test" },
  "projects": {
    "C:/Code/demo": {
      "lastCost": 1.25,
      "allowedTools": ["Read"],
      "obsolete": true,
      "lastSessionId": "session-default"
    }
  }
}
"#;
    write(&world.state("default"), ours);
    write(
        &world.state("team"),
        &state(
            "team@example.test",
            json!({"allowedTools": ["Read"], "obsolete": true, "lastCost": 9.5}),
        ),
    );
    world.synced(&["team"], 1);
    assert_eq!(read(&world.state("default")), ours);
    // The profile changes a real field, drops another and adds a project of its own.
    let mut team = parsed(&world.state("team"));
    team["projects"]["C:/Code/demo"] = json!({
        "allowedTools": ["Read", "Edit"], "lastCost": 20, "lastSessionId": "session-team",
        "hasTrustDialogAccepted": true
    });
    team["projects"]["C:/Code/new"] = json!({
        "allowedTools": ["Bash"], "lastCost": 3, "hasUnseenTeamArtifacts": true
    });
    let theirs = serde_json::to_string_pretty(&team).unwrap();
    write(&world.state("team"), &theirs);
    touch(&world.state("default"), 100);
    touch(&world.state("team"), 200);
    world.synced(&["team", "solo"], 2);
    // The winning side is not rewritten at all.
    assert_eq!(read(&world.state("team")), theirs);
    let default = read(&world.state("default"));
    let value: Value = serde_json::from_str(&default).unwrap();
    // The real fields are the profile's, the runtime fields still the default home's — in
    // the default home's order, the new field last.
    assert_eq!(
        value["projects"]["C:/Code/demo"],
        json!({
            "lastCost": 1.25, "allowedTools": ["Read", "Edit"],
            "lastSessionId": "session-default", "hasTrustDialogAccepted": true
        })
    );
    let places: Vec<_> = [
        "\"lastCost\"",
        "\"allowedTools\"",
        "\"lastSessionId\"",
        "\"hasTrustDialogAccepted\"",
    ]
    .iter()
    .map(|field| default.find(field).unwrap())
    .collect();
    assert!(places.is_sorted(), "{default}");
    // An entry only one side had arrives without its runtime fields.
    assert_eq!(
        value["projects"]["C:/Code/new"],
        json!({"allowedTools": ["Bash"]})
    );
    // Every other key keeps its exact text.
    assert!(default.contains("\"bigNumber\": 12345678901234567890.5000,"));
    assert!(default.contains(r#""oauthAccount": { "emailAddress": "max@example.test" },"#));
    // A new profile's state file is created with the projects, never with runtime fields.
    let solo = read(&world.state("solo"));
    assert!(
        !solo.contains("last") && !solo.contains("hasUnseenTeamArtifacts"),
        "{solo}"
    );
    assert_eq!(
        parsed(&world.state("solo"))["projects"],
        json!({
            "C:/Code/demo": {"allowedTools": ["Read", "Edit"], "hasTrustDialogAccepted": true},
            "C:/Code/new": {"allowedTools": ["Bash"]}
        })
    );
    // A fixed point: nothing is written again.
    let before = (stamp(&world.state("default")), stamp(&world.state("solo")));
    world.synced(&["team", "solo"], 3);
    assert_eq!(stamp(&world.state("default")), before.0);
    assert_eq!(stamp(&world.state("solo")), before.1);
    assert_eq!(read(&world.state("team")), theirs);
}
