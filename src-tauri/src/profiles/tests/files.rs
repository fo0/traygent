//! The synced files, the history, the state file's shared keys and what is never touched.
use super::*;
use crate::profiles::history::union;
use crate::profiles::json::{merge_keys, NotJson};
use serde_json::{json, Value};

const DEFAULT_STATE: &str = r#"{
  "numStartups": 5,
  "oauthAccount": {
    "emailAddress": "max@example.test"
  },
  "mcpServers": {
    "docs": {
      "command": "docs-mcp"
    }
  },
  "projects": {
    "C:/Code/demo": {
      "hasTrustDialogAccepted": true
    }
  }
}
"#;
const TEAM_STATE: &str = r#"{
  "installMethod": "native",
  "oauthAccount": {
    "emailAddress": "team@example.test",
    "organizationName": "Team GmbH"
  },
  "mcpServers": {},
  "bigNumber": 12345678901234567890.5000,
  "numStartups": 1
}
"#;

fn parsed(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap()
}

#[test]
fn the_first_sync_takes_the_default_homes_files_and_backs_the_profiles_up() {
    let world = World::new("profiles-first");
    let (ours, theirs) = (world.dir("default"), world.dir("team"));
    write(&ours.join("settings.json"), r#"{"theme":"dark"}"#);
    write(&theirs.join("settings.json"), r#"{"theme":"light"}"#);
    write(&ours.join("CLAUDE.md"), "same");
    write(&theirs.join("CLAUDE.md"), "same");
    write(&theirs.join("keybindings.json"), "{}");
    // Even a newer profile file loses the first time.
    touch(&ours.join("settings.json"), 10);
    touch(&theirs.join("settings.json"), 500);
    let reports = world.synced(&["team"], 1);
    assert!(reports[0].changed);
    assert_eq!(read(&theirs.join("settings.json")), r#"{"theme":"dark"}"#);
    assert_eq!(read(&ours.join("settings.json")), r#"{"theme":"dark"}"#);
    let backup = world.backup("team");
    assert_eq!(read(&backup.join("settings.json")), r#"{"theme":"light"}"#);
    // Equal content needs no backup; a file on one side only is copied over.
    assert!(!backup.join("CLAUDE.md").exists());
    assert_eq!(read(&ours.join("keybindings.json")), "{}");
}

#[test]
fn later_the_newer_file_wins_in_either_direction() {
    let world = World::new("profiles-newest");
    let (ours, theirs) = (world.dir("default"), world.dir("team"));
    write(&ours.join("settings.json"), "v1");
    write(&ours.join("CLAUDE.md"), "v1");
    world.synced(&["team"], 1);
    assert_eq!(read(&theirs.join("settings.json")), "v1");
    // The profile's file is newer: it reaches the default home.
    write(&theirs.join("settings.json"), "team edit");
    touch(&ours.join("settings.json"), 100);
    touch(&theirs.join("settings.json"), 200);
    // The default home's file is newer: it reaches the profile.
    write(&ours.join("CLAUDE.md"), "default edit");
    touch(&ours.join("CLAUDE.md"), 400);
    touch(&theirs.join("CLAUDE.md"), 300);
    world.synced(&["team"], 2);
    assert_eq!(read(&ours.join("settings.json")), "team edit");
    assert_eq!(read(&theirs.join("settings.json")), "team edit");
    assert_eq!(read(&ours.join("CLAUDE.md")), "default edit");
    assert_eq!(read(&theirs.join("CLAUDE.md")), "default edit");
    // Later syncs replace without a backup.
    assert!(!theirs.join(super::super::files::BACKUP_DIR).exists());
    // A file deleted on one side comes back from the other.
    std::fs::remove_file(ours.join("CLAUDE.md")).unwrap();
    world.synced(&["team"], 3);
    assert_eq!(read(&ours.join("CLAUDE.md")), "default edit");
}

#[test]
fn a_change_in_one_profile_reaches_the_others_in_the_same_run() {
    let world = World::new("profiles-three");
    write(&world.dir("default").join("settings.json"), "v1");
    world.synced(&["team", "solo"], 1);
    for id in ["default", "team", "solo"] {
        touch(&world.dir(id).join("settings.json"), 100);
    }
    write(&world.dir("solo").join("settings.json"), "solo edit");
    touch(&world.dir("solo").join("settings.json"), 900);
    world.synced(&["team", "solo"], 2);
    for id in ["default", "team", "solo"] {
        assert_eq!(
            read(&world.dir(id).join("settings.json")),
            "solo edit",
            "{id}"
        );
    }
}

#[test]
fn history_lines_are_united_in_timestamp_order_without_duplicates() {
    let line = |at: u32, text: &str| format!(r#"{{"display":"{text}","timestamp":{at}}}"#);
    let ours = [line(1, "a"), line(3, "c"), line(5, "e")].join("\n") + "\n";
    let theirs = [line(2, "b"), line(3, "c"), "not json".into(), line(4, "d")].join("\r\n");
    let merged = String::from_utf8(union(ours.as_bytes(), theirs.as_bytes())).unwrap();
    let expected = [
        line(1, "a"),
        line(2, "b"),
        line(3, "c"),
        // A line without a timestamp keeps its place among its file's lines.
        "not json".into(),
        line(4, "d"),
        line(5, "e"),
    ];
    assert_eq!(merged, expected.join("\n") + "\n");
    // A file that already is the union stays byte for byte.
    assert_eq!(
        union(merged.as_bytes(), theirs.as_bytes()),
        merged.as_bytes()
    );
    assert_eq!(union(merged.as_bytes(), b""), merged.as_bytes());
    assert_eq!(union(b"", b""), b"");

    let world = World::new("profiles-history");
    let (ours_file, theirs_file) = (
        world.dir("default").join("history.jsonl"),
        world.dir("team").join("history.jsonl"),
    );
    write(&ours_file, &ours);
    write(&theirs_file, &theirs);
    world.synced(&["team"], 1);
    assert_eq!(read(&ours_file), merged);
    assert_eq!(read(&theirs_file), merged);
    // Only one side has a history: the other gets it.
    let solo = World::new("profiles-history-solo");
    write(&solo.dir("team").join("history.jsonl"), &ours);
    solo.synced(&["team"], 1);
    assert_eq!(read(&solo.dir("default").join("history.jsonl")), ours);
}

#[test]
fn only_the_shared_keys_of_the_state_file_travel() {
    let world = World::new("profiles-state");
    write(&world.state("default"), DEFAULT_STATE);
    write(&world.state("team"), TEAM_STATE);
    // First sync: from the default home, even though the profile's file is newer.
    touch(&world.state("default"), 10);
    touch(&world.state("team"), 500);
    world.synced(&["team"], 1);
    assert_eq!(
        read(&world.state("default")),
        DEFAULT_STATE,
        "the source is not written"
    );
    let team = read(&world.state("team"));
    let value: Value = serde_json::from_str(&team).unwrap();
    let default_value: Value = serde_json::from_str(DEFAULT_STATE).unwrap();
    assert_eq!(value["mcpServers"], default_value["mcpServers"]);
    assert_eq!(value["projects"], default_value["projects"]);
    // Identity and everything else stay the profile's own.
    assert_eq!(value["oauthAccount"]["emailAddress"], "team@example.test");
    assert_eq!(value["oauthAccount"]["organizationName"], "Team GmbH");
    assert_eq!(value["numStartups"], 1);
    assert_eq!(value["installMethod"], "native");
    // Untouched values keep their text, the keys their order; a new key goes last.
    assert!(team.contains("\"bigNumber\": 12345678901234567890.5000,"));
    let order: Vec<usize> = [
        "installMethod",
        "oauthAccount",
        "mcpServers",
        "bigNumber",
        "numStartups",
        "projects",
    ]
    .iter()
    .map(|key| team.find(&format!("\n  \"{key}\":")).unwrap())
    .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{team}");
    assert!(team.ends_with("}\n"));

    // Later the newer file is the source: a server added in the profile reaches the default.
    let mut edited: Value = serde_json::from_str(&team).unwrap();
    edited["mcpServers"]["tickets"] = json!({ "command": "tickets-mcp" });
    write(
        &world.state("team"),
        &serde_json::to_string_pretty(&edited).unwrap(),
    );
    touch(&world.state("default"), 600);
    touch(&world.state("team"), 700);
    world.synced(&["team"], 2);
    let ours = parsed(&world.state("default"));
    assert_eq!(ours["mcpServers"]["tickets"]["command"], "tickets-mcp");
    assert_eq!(ours["oauthAccount"]["emailAddress"], "max@example.test");
    assert_eq!(ours["numStartups"], 5);
    assert_eq!(
        parsed(&world.state("team"))["oauthAccount"]["emailAddress"],
        "team@example.test"
    );
}

#[test]
fn key_merges_keep_what_the_source_lacks_and_ignore_formatting() {
    let keys = ["mcpServers", "projects"];
    // A key the source lacks stays as the target has it.
    let merged = merge_keys(
        br#"{"mcpServers":{"a":1}}"#,
        Some(br#"{"projects":{"p":1},"x":true}"#),
        &keys,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        String::from_utf8(merged).unwrap(),
        r#"{"projects":{"p":1},"x":true,"mcpServers":{"a":1}}"#
    );
    // The same values in another layout are no change.
    let same = merge_keys(
        b"{\"mcpServers\": {\"a\": 1,\n \"b\": [1, 2]}}",
        Some(br#"{"mcpServers":{"b":[1,2],"a":1},"id":"keep"}"#),
        &keys,
    );
    assert_eq!(same, Ok(None));
    assert_eq!(merge_keys(b"{}", Some(b"{}"), &keys), Ok(None));
    // A profile without a state file gets one with the shared keys only.
    let created = merge_keys(
        br#"{"mcpServers":{"a":1},"oauthAccount":{"emailAddress":"max@example.test"}}"#,
        None,
        &keys,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        String::from_utf8(created).unwrap(),
        "{\n  \"mcpServers\": {\"a\":1}\n}"
    );
    // Anything that is no JSON object stops the merge.
    for broken in ["", "{ nope", "[1]", "null", "{\"a\":1} trailing"] {
        assert_eq!(
            merge_keys(broken.as_bytes(), Some(b"{}"), &keys),
            Err(NotJson),
            "{broken}"
        );
        assert_eq!(
            merge_keys(b"{\"mcpServers\":{}}", Some(broken.as_bytes()), &keys),
            Err(NotJson)
        );
    }
}

#[test]
fn an_unparsable_state_file_is_never_overwritten() {
    for broken_side in ["default", "team"] {
        let world = World::new("profiles-broken");
        write(&world.state("default"), DEFAULT_STATE);
        write(&world.state("team"), TEAM_STATE);
        write(&world.state(broken_side), "{ \"mcpServers\": ");
        let other = if broken_side == "default" {
            "team"
        } else {
            "default"
        };
        let before = read(&world.state(other));
        let reports = world.sync(&["team"], 1);
        assert_eq!(
            reports[0].errors,
            [".claude.json: ist kein lesbares JSON und bleibt unverändert"]
        );
        assert_eq!(read(&world.state(broken_side)), "{ \"mcpServers\": ");
        assert_eq!(read(&world.state(other)), before);
        // The rest of the profile is synced.
        assert!(world.dir("team").join("skills").exists());
    }
}

#[test]
fn credentials_never_leave_their_profile() {
    let world = World::new("profiles-credentials");
    let (default_secret, team_secret) =
        ("synthetic-default-credential", "synthetic-team-credential");
    write(
        &world.dir("default").join(".credentials.json"),
        default_secret,
    );
    write(&world.dir("team").join(".credentials.json"), team_secret);
    // Files no rule lists stay where they are, too.
    write(
        &world.dir("default").join("statsig/cache.json"),
        "default cache",
    );
    write(&world.dir("team").join("todos/1.json"), "team todo");
    write(&world.dir("default").join("settings.json"), "{}");
    write(&world.state("default"), DEFAULT_STATE);
    write(&world.state("team"), TEAM_STATE);
    for minute in [1, 2] {
        world.synced(&["team", "solo"], minute);
    }
    assert_eq!(
        read(&world.dir("default").join(".credentials.json")),
        default_secret
    );
    assert_eq!(
        read(&world.dir("team").join(".credentials.json")),
        team_secret
    );
    assert!(!world.dir("solo").join(".credentials.json").exists());
    assert!(!world.dir("solo").join("statsig").exists());
    assert!(!world.dir("default").join("todos").exists());
    // Each secret exists exactly once under the whole user profile, backups included.
    let holders = |secret: &str| -> Vec<PathBuf> {
        tree(world.home())
            .into_iter()
            .filter(|(_, bytes, _)| String::from_utf8_lossy(bytes).contains(secret))
            .map(|(path, _, _)| path)
            .collect()
    };
    assert_eq!(
        holders(default_secret),
        [world.dir("default").join(".credentials.json")]
    );
    assert_eq!(
        holders(team_secret),
        [world.dir("team").join(".credentials.json")]
    );
}

#[test]
fn a_second_run_writes_nothing_but_the_marker() {
    let world = World::new("profiles-idempotent");
    let (ours, theirs) = (world.dir("default"), world.dir("team"));
    write(&ours.join("settings.json"), r#"{"theme":"dark"}"#);
    write(&theirs.join("settings.json"), r#"{"theme":"light"}"#);
    write(
        &ours.join("history.jsonl"),
        "{\"timestamp\":1}\n{\"timestamp\":3}\n",
    );
    write(&theirs.join("history.jsonl"), "{\"timestamp\":2}\n");
    write(&theirs.join("skills/team.md"), "team skill");
    write(&ours.join("projects/demo/session.jsonl"), "{}\n");
    write(&world.state("default"), DEFAULT_STATE);
    write(&world.state("team"), TEAM_STATE);
    let first = world.synced(&["team", "solo"], 1);
    assert!(first.iter().all(|report| report.changed));
    let without_markers = || -> Vec<_> {
        tree(world.home())
            .into_iter()
            .filter(|(path, _, _)| path.file_name().unwrap() != MARKER)
            .collect()
    };
    let before = without_markers();
    let second = world.synced(&["team", "solo"], 30);
    assert!(second.iter().all(|report| !report.changed), "{second:?}");
    assert_eq!(
        without_markers(),
        before,
        "content and file times are untouched"
    );
    let marker = read_marker(&theirs).unwrap();
    assert_eq!(marker.synced_at.as_deref(), Some("2026-11-13T09:30:00Z"));
}
