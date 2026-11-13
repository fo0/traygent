//! The synced files, the history, the state file's shared keys and what is never touched.
use super::*;
use crate::profiles::history::union;

#[test]
fn the_first_sync_takes_the_default_homes_files_and_backs_the_profiles_up() {
    let world = World::new("profiles-first");
    let (ours, theirs) = (world.dir("default"), world.dir("team"));
    write(&ours.join("settings.json"), r#"{"theme":"dark"}"#);
    own(&world, "team");
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
    // Whatever was replaced is in the backup of the home it was in.
    assert_eq!(read(&world.backup("default").join("settings.json")), "v1");
    assert_eq!(read(&world.backup("team").join("CLAUDE.md")), "v1");
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
    own(&world, "team");
    world.synced(&["team"], 1);
    assert_eq!(read(&ours_file), merged);
    assert_eq!(read(&theirs_file), merged);
    // Only one side has a history: the other gets it.
    let solo = World::new("profiles-history-solo");
    write(&solo.dir("team").join("history.jsonl"), &ours);
    own(&solo, "team");
    solo.synced(&["team"], 1);
    assert_eq!(read(&solo.dir("default").join("history.jsonl")), ours);
}

/// A rule that failed has not had its first sync: the default home still wins the next
/// time, and what loses is backed up — on either side, first sync or not.
#[test]
fn a_failed_rule_stays_a_first_sync_and_every_loser_is_backed_up() {
    let world = World::new("profiles-first-failed");
    let (ours, theirs) = (world.dir("default"), world.dir("team"));
    write(&ours.join("CLAUDE.md"), "default rules");
    write(&ours.join("settings.json"), "default settings");
    own(&world, "team");
    // Something no rule can write to: a directory where the profile's file belongs.
    write(&theirs.join("CLAUDE.md/inner"), "x");
    let reports = world.sync(&["team"], 1);
    assert_eq!(reports[0].errors, ["CLAUDE.md: ist keine Datei"]);
    let marker = read_marker(&theirs).unwrap();
    assert_eq!(marker.synced_at, None);
    assert!(
        !marker.done.iter().any(|rule| rule == "CLAUDE.md"),
        "{:?}",
        marker.done
    );
    assert!(marker.done.iter().any(|rule| rule == "settings.json"));
    // The profile now has a real, newer file. Newest-wins would replace the default home's
    // rules; the rule's first sync is still due, so the default home wins.
    std::fs::remove_dir_all(theirs.join("CLAUDE.md")).unwrap();
    write(&theirs.join("CLAUDE.md"), "team rules");
    touch(&ours.join("CLAUDE.md"), 10);
    touch(&theirs.join("CLAUDE.md"), 900);
    world.synced(&["team"], 2);
    assert_eq!(read(&ours.join("CLAUDE.md")), "default rules");
    assert_eq!(read(&theirs.join("CLAUDE.md")), "default rules");
    assert_eq!(read(&world.backup("team").join("CLAUDE.md")), "team rules");
    assert!(read_marker(&theirs)
        .unwrap()
        .done
        .iter()
        .any(|rule| rule == "CLAUDE.md"));
    // From now on the newer file wins — and the default home's loser is kept in its backup.
    write(&theirs.join("settings.json"), "team settings");
    touch(&ours.join("settings.json"), 10);
    touch(&theirs.join("settings.json"), 900);
    world.synced(&["team"], 3);
    assert_eq!(read(&ours.join("settings.json")), "team settings");
    assert_eq!(
        read(&world.backup("default").join("settings.json")),
        "default settings"
    );
}

/// The CLI appends to its files while a session runs: what arrived since a file was read
/// is never replaced by a merge of the older read.
#[test]
fn a_file_that_changed_since_it_was_read_is_noticed() {
    use crate::profiles::files::{load, unchanged};
    let world = World::new("profiles-meanwhile");
    let file = world.dir("default").join("history.jsonl");
    assert!(unchanged(&file, None));
    write(&file, "{\"timestamp\":1}\n");
    assert!(!unchanged(&file, None), "it appeared");
    touch(&file, 10);
    let seen = load(&file, 1024).unwrap();
    assert!(unchanged(&file, seen.as_ref()));
    // A line appended (other size) or a rewrite of the same size (other time).
    write(&file, "{\"timestamp\":1}\n{\"timestamp\":2}\n");
    touch(&file, 10);
    assert!(!unchanged(&file, seen.as_ref()));
    write(&file, "{\"timestamp\":7}\n");
    touch(&file, 11);
    assert!(!unchanged(&file, seen.as_ref()));
    std::fs::remove_file(&file).unwrap();
    assert!(!unchanged(&file, seen.as_ref()));
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
