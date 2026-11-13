//! The settings rule (#35): a three-way merge of the shared part, the sign-in keys staying
//! in their profile. Every organization id, key and helper here is made up.
use super::*;
use crate::profiles::settings::BASE_DIR;
use serde_json::{json, Value};

mod guards;
mod safety;

pub(super) const FILE: &str = "settings.json";

pub(super) fn settings(world: &World, id: &str) -> PathBuf {
    world.dir(id).join(FILE)
}

pub(super) fn base(world: &World, id: &str) -> PathBuf {
    world.dir(id).join(BASE_DIR).join(FILE)
}

pub(super) fn parsed(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap()
}

/// Writes a side's settings with a fixed file time.
pub(super) fn put(world: &World, id: &str, value: Value, seconds: u64) {
    write(&settings(world, id), &value.to_string());
    touch(&settings(world, id), seconds);
}

/// Every file under the user profile that holds `text`.
fn holders(world: &World, text: &str) -> Vec<PathBuf> {
    let files = tree(world.home()).into_iter();
    let found = files.filter(|(_, bytes, _)| String::from_utf8_lossy(bytes).contains(text));
    found.map(|(path, _, _)| path).collect()
}

#[test]
fn force_login_org_uuid_and_env_api_key_never_cross_profiles_or_reach_the_base() {
    let world = World::new("settings-own");
    let default = json!({
        "forceLoginOrgUUID": "org-default-synthetic",
        "env": {"ANTHROPIC_API_KEY": "key-default-synthetic", "EDITOR": "vim"},
        "theme": "dark"
    });
    let team = json!({
        "theme": "dark",
        "forceLoginOrgUUID": "org-team-synthetic",
        "apiKeyHelper": "helper-team-synthetic",
        "otelHeadersHelper": "otel-helper-team-synthetic",
        "env": {
            "ANTHROPIC_API_KEY": "key-team-synthetic",
            "aws_profile": "aws-team-synthetic",
            "OTEL_EXPORTER_OTLP_HEADERS": "otel-team-synthetic",
            "VERTEX_REGION_CLAUDE_4_0_SONNET": "vertex-team-synthetic",
            "CLAUDE_CODE_CLIENT_KEY": "mtls-team-synthetic",
            "CLOUD_ML_REGION": "region-team-synthetic",
            "CLAUDE_CODE_SKIP_BEDROCK_AUTH": "skip-team-synthetic",
            "HTTPS_PROXY": "http://proxy.example.test:8080"
        }
    });
    // One of every kind of account- or organisation-bound setting.
    const TEAM_ONLY: [&str; 9] = [
        "helper-team-synthetic",
        "otel-helper-team-synthetic",
        "key-team-synthetic",
        "aws-team-synthetic",
        "otel-team-synthetic",
        "vertex-team-synthetic",
        "mtls-team-synthetic",
        "region-team-synthetic",
        "skip-team-synthetic",
    ];
    let team_only = || TEAM_ONLY.map(|secret| ("team", secret));
    put(&world, "default", default, 10);
    put(&world, "team", team, 20);
    put(&world, "solo", json!({"model": "opus"}), 30);
    own(&world, "team");
    own(&world, "solo");
    // Every secret stays in files of its own home (the file and that home's backup), and
    // no base holds a key of the sign-in — its name or its value.
    let check = |secrets: &[(&str, &str)]| {
        for (id, secret) in secrets {
            let found = holders(&world, secret);
            assert!(!found.is_empty(), "{secret}");
            for path in found {
                assert!(path.starts_with(world.dir(id)), "{secret} in {path:?}");
            }
        }
        for id in ["team", "solo"] {
            let text = read(&base(&world, id)).to_lowercase();
            for word in [
                "forcelogin",
                "anthropic_",
                "apikeyhelper",
                "aws_",
                "otel",
                "vertex_region",
                "claude_code_client",
                "cloud_ml_region",
                "claude_code_skip",
                "synthetic",
            ] {
                assert!(!text.contains(word), "{word} in the base of {id}");
            }
            assert!(text.contains("editor") && text.contains("opus"));
        }
        assert!(!base(&world, "default").exists());
    };

    world.synced(&["team", "solo"], 1);
    check(&[
        ("default", "org-default-synthetic"),
        ("default", "key-default-synthetic"),
        ("team", "org-team-synthetic"),
    ]);
    check(&team_only());
    // The shared part did travel; a profile without the keys stays without them.
    assert_eq!(
        parsed(&settings(&world, "solo")),
        json!({
            "model": "opus",
            // The proxy describes the machine, not the account: it is shared.
            "env": {"EDITOR": "vim", "HTTPS_PROXY": "http://proxy.example.test:8080"},
            "theme": "dark"
        })
    );
    let team_env = parsed(&settings(&world, "team"))["env"].clone();
    assert_eq!(team_env.as_object().unwrap().len(), 9);
    assert_eq!(team_env["EDITOR"], "vim");
    assert_eq!(team_env["CLOUD_ML_REGION"], "region-team-synthetic");
    assert_eq!(parsed(&settings(&world, "default"))["model"], "opus");

    // Later syncs: a changed, a removed and a new own key each stay where they are.
    let edit = |id: &str, change: &dyn Fn(&mut Value)| {
        let mut value = parsed(&settings(&world, id));
        change(&mut value);
        write(&settings(&world, id), &value.to_string());
    };
    edit("team", &|value| {
        value["forceLoginOrgUUID"] = json!("org-team-2-synthetic");
        value["verbose"] = json!(true);
    });
    edit("default", &|value| {
        value.as_object_mut().unwrap().remove("forceLoginOrgUUID");
    });
    edit("solo", &|value| {
        value["env"]["ANTHROPIC_API_KEY"] = json!("key-solo-synthetic");
    });
    for minute in [2, 3] {
        world.synced(&["team", "solo"], minute);
        check(&[
            // Replaced or removed values live on in their own home's backup only.
            ("default", "org-default-synthetic"),
            ("team", "org-team-synthetic"),
            ("default", "key-default-synthetic"),
            ("team", "org-team-2-synthetic"),
            ("team", "helper-team-synthetic"),
            ("team", "key-team-synthetic"),
            ("solo", "key-solo-synthetic"),
        ]);
        check(&team_only()[1..]);
        let (ours, team, solo) = (
            parsed(&settings(&world, "default")),
            parsed(&settings(&world, "team")),
            parsed(&settings(&world, "solo")),
        );
        assert_eq!(ours.get("forceLoginOrgUUID"), None);
        assert_eq!(solo.get("forceLoginOrgUUID"), None);
        assert_eq!(team["forceLoginOrgUUID"], "org-team-2-synthetic");
        assert_eq!(ours["env"]["ANTHROPIC_API_KEY"], "key-default-synthetic");
        assert_eq!(solo["env"]["ANTHROPIC_API_KEY"], "key-solo-synthetic");
        for side in [&ours, &team, &solo] {
            assert_eq!(side["verbose"], true);
        }
    }
}

#[test]
fn the_first_sync_unites_and_the_default_home_wins_a_conflict() {
    let world = World::new("settings-first");
    let ours = json!({"theme": "dark", "permissions": {"allow": ["A"]}, "onlyDefault": 1});
    let theirs = json!({"theme": "light", "permissions": {"allow": ["B"]}, "onlyTeam": 2});
    put(&world, "default", ours.clone(), 10);
    // Even a newer profile file loses the first time.
    put(&world, "team", theirs.clone(), 500);
    own(&world, "team");
    world.synced(&["team"], 1);
    let merged = json!({
        "theme": "dark",
        "permissions": {"allow": ["A", "B"]},
        "onlyDefault": 1,
        "onlyTeam": 2
    });
    for id in ["default", "team"] {
        assert_eq!(parsed(&settings(&world, id)), merged, "{id}");
    }
    assert_eq!(parsed(&base(&world, "team")), merged);
    // Each side that the first sync rewrote is in its own home's backup.
    assert_eq!(parsed(&world.backup("default").join(FILE)), ours);
    assert_eq!(parsed(&world.backup("team").join(FILE)), theirs);
}

#[test]
fn a_profile_synced_by_the_previous_version_starts_without_a_base() {
    let world = World::new("settings-upgrade");
    put(&world, "default", json!({"theme": "dark", "a": 1}), 10);
    put(&world, "team", json!({"theme": "light", "b": 2}), 900);
    write(
        &world.dir("team").join(MARKER),
        r#"{"version":1,"syncedAt":"2026-11-13T08:00:00Z","done":["settings.json","CLAUDE.md","keybindings.json",".claude.json"]}"#,
    );
    world.synced(&["team"], 1);
    // No base: nothing is removed. The rule has had its first sync, so the newer file
    // wins as it did in the version that wrote the marker — here the profile's.
    let merged = json!({"theme": "light", "a": 1, "b": 2});
    assert_eq!(parsed(&settings(&world, "default")), merged);
    assert_eq!(parsed(&settings(&world, "team")), merged);
    assert_eq!(parsed(&base(&world, "team")), merged);
    // The conflict went against the default home: its file is kept. The profile only
    // gained a key.
    assert_eq!(
        parsed(&world.backup("default").join(FILE)),
        json!({"theme": "dark", "a": 1})
    );
    assert!(!world.dir("team").join(".traygent-backup").exists());
    // The other way round the default home's newer file wins.
    let older = World::new("settings-upgrade-default");
    put(&older, "default", json!({"theme": "dark"}), 900);
    put(&older, "team", json!({"theme": "light"}), 10);
    write(
        &older.dir("team").join(MARKER),
        r#"{"version":1,"done":["settings.json"]}"#,
    );
    older.synced(&["team"], 1);
    assert_eq!(parsed(&settings(&older, "team"))["theme"], "dark");
    // With the base recorded a removal travels.
    put(&world, "team", json!({"theme": "light", "b": 2}), 950);
    world.synced(&["team"], 2);
    assert_eq!(
        parsed(&settings(&world, "default")),
        json!({"theme": "light", "b": 2})
    );
}

#[test]
fn permission_lists_merge_as_sets_with_additions_and_removals() {
    let world = World::new("settings-sets");
    let with = |allow: Value| json!({"permissions": {"allow": allow, "defaultMode": "plan"}});
    put(&world, "default", with(json!(["Bash(ls)", "Read"])), 10);
    world.synced(&["team"], 1);
    assert_eq!(
        parsed(&settings(&world, "team")),
        with(json!(["Bash(ls)", "Read"]))
    );
    // The default home adds one; the profile removes one and adds another.
    put(
        &world,
        "default",
        with(json!(["Bash(ls)", "Read", "Edit"])),
        100,
    );
    put(&world, "team", with(json!(["Read", "WebFetch"])), 200);
    world.synced(&["team"], 2);
    let merged = with(json!(["Read", "Edit", "WebFetch"]));
    assert_eq!(parsed(&settings(&world, "default")), merged);
    assert_eq!(parsed(&settings(&world, "team")), merged);
    assert_eq!(parsed(&base(&world, "team")), merged);
    // The side that lost an item is in its backup; the other only gained one.
    assert_eq!(
        parsed(&world.backup("default").join(FILE)),
        with(json!(["Bash(ls)", "Read", "Edit"]))
    );
    assert!(!world.dir("team").join(".traygent-backup").exists());
}

#[test]
fn a_removed_shared_key_is_removed_on_the_other_side() {
    let world = World::new("settings-removal");
    let before = json!({
        "theme": "dark",
        "verbose": true,
        "statusLine": {"type": "command", "padding": 0}
    });
    put(&world, "default", before.clone(), 10);
    world.synced(&["team"], 1);
    let after = json!({"theme": "dark", "statusLine": {"type": "command"}});
    put(&world, "team", after.clone(), 200);
    world.synced(&["team"], 2);
    assert_eq!(parsed(&settings(&world, "default")), after);
    assert_eq!(parsed(&base(&world, "team")), after);
    assert_eq!(parsed(&world.backup("default").join(FILE)), before);
    // It stays removed.
    world.synced(&["team"], 3);
    assert_eq!(parsed(&settings(&world, "default")), after);
    assert_eq!(parsed(&settings(&world, "team")), after);
}

#[test]
fn a_conflict_goes_to_the_newer_file_and_the_loser_to_its_backup() {
    let world = World::new("settings-conflict");
    put(
        &world,
        "default",
        json!({"theme": "dark", "model": "sonnet"}),
        10,
    );
    world.synced(&["team"], 1);
    // Both changed `theme`; only the default home changed `model`.
    let ours = json!({"theme": "light", "model": "opus"});
    put(&world, "default", ours.clone(), 100);
    put(
        &world,
        "team",
        json!({"theme": "blue", "model": "sonnet"}),
        200,
    );
    world.synced(&["team"], 2);
    let merged = json!({"theme": "blue", "model": "opus"});
    assert_eq!(parsed(&settings(&world, "default")), merged);
    assert_eq!(parsed(&settings(&world, "team")), merged);
    assert_eq!(parsed(&world.backup("default").join(FILE)), ours);
    // Both files carry the time of the newest content merged into them, not of the sync.
    let newest = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_200);
    for id in ["default", "team"] {
        let modified = std::fs::metadata(settings(&world, id)).unwrap().modified();
        assert_eq!(modified.unwrap(), newest, "{id}");
    }
    // The profile took a change without losing anything: no backup.
    assert!(!world.dir("team").join(".traygent-backup").exists());
    // The other direction.
    put(
        &world,
        "default",
        json!({"theme": "red", "model": "opus"}),
        900,
    );
    let theirs = json!({"theme": "green", "model": "opus"});
    put(&world, "team", theirs.clone(), 800);
    world.synced(&["team"], 3);
    assert_eq!(parsed(&settings(&world, "team"))["theme"], "red");
    assert_eq!(parsed(&world.backup("team").join(FILE)), theirs);
}

#[test]
fn each_side_keeps_its_key_order_and_new_keys_are_appended() {
    let world = World::new("settings-order");
    write(
        &settings(&world, "default"),
        r#"{"zeta":1,"alpha":{"b":1,"a":2},"env":{"Z":"1","ANTHROPIC_MODEL":"model-default","A":"2"}}"#,
    );
    write(
        &settings(&world, "team"),
        r#"{"env":{"A":"2","B":"3"},"mid":true,"alpha":{"a":2,"b":1},"forceLoginMethod":"console"}"#,
    );
    own(&world, "team");
    world.synced(&["team"], 1);
    assert_eq!(
        read(&settings(&world, "default")),
        r#"{
  "zeta": 1,
  "alpha": {
    "b": 1,
    "a": 2
  },
  "env": {
    "Z": "1",
    "ANTHROPIC_MODEL": "model-default",
    "A": "2",
    "B": "3"
  },
  "mid": true
}
"#
    );
    assert_eq!(
        read(&settings(&world, "team")),
        r#"{
  "env": {
    "A": "2",
    "B": "3",
    "Z": "1"
  },
  "mid": true,
  "alpha": {
    "a": 2,
    "b": 1
  },
  "forceLoginMethod": "console",
  "zeta": 1
}
"#
    );
}

#[test]
fn formatting_and_key_order_alone_cause_no_write() {
    let world = World::new("settings-format");
    let compact = r#"{"a":1,"b":{"c":[1,2],"d":"x"},"env":{}}"#;
    let spread =
        "{\r\n\t\"b\": {\"d\": \"x\", \"c\": [1, 2]},\r\n\t\"a\": 1,\r\n\t\"env\": {}\r\n}";
    write(&settings(&world, "default"), compact);
    write(&settings(&world, "team"), spread);
    touch(&settings(&world, "default"), 10);
    touch(&settings(&world, "team"), 500);
    own(&world, "team");
    let before = (tree(&world.dir("default")), tree(&world.dir("team")));
    world.synced(&["team"], 1);
    assert_eq!(read(&settings(&world, "default")), compact);
    assert_eq!(read(&settings(&world, "team")), spread);
    assert_eq!(
        tree(&world.dir("default")),
        before.0,
        "content and file times"
    );
    assert!(!world.dir("team").join(".traygent-backup").exists());
    // The base is recorded all the same, and the second run finds nothing to do.
    assert_eq!(
        parsed(&base(&world, "team")),
        json!({"a": 1, "b": {"c": [1, 2], "d": "x"}})
    );
    let reports = world.synced(&["team"], 2);
    assert!(!reports[0].changed);
    assert_eq!(read(&settings(&world, "team")), spread);
}

#[test]
fn three_profiles_reach_a_fixed_point_in_one_run() {
    let world = World::new("settings-three");
    let ids = ["team", "solo", "lab"];
    put(
        &world,
        "default",
        json!({"theme": "dark", "permissions": {"allow": ["A"]}}),
        10,
    );
    world.synced(&ids, 1);
    // Every home changes something else between two syncs.
    let allow = |items: Value| json!({"allow": items});
    put(
        &world,
        "default",
        json!({"theme": "dark", "permissions": allow(json!(["A", "D"]))}),
        100,
    );
    put(
        &world,
        "team",
        json!({"theme": "dark", "permissions": allow(json!(["A", "T"])), "model": "opus"}),
        200,
    );
    put(
        &world,
        "solo",
        json!({"permissions": allow(json!(["A", "S"]))}),
        300,
    );
    put(
        &world,
        "lab",
        json!({"theme": "dark", "permissions": allow(json!([])), "verbose": true}),
        400,
    );
    world.synced(&ids, 2);
    let merged = json!({
        "permissions": allow(json!(["D", "T", "S"])),
        "model": "opus",
        "verbose": true
    });
    for id in ["default", "team", "solo", "lab"] {
        assert_eq!(parsed(&settings(&world, id)), merged, "{id}");
    }
    for id in ids {
        assert_eq!(parsed(&base(&world, id)), merged, "base of {id}");
    }
    // A fixed point: the next run writes nothing but the markers.
    let without_markers = || -> Vec<_> {
        let files = tree(world.home()).into_iter();
        files
            .filter(|(path, _, _)| path.file_name().unwrap() != MARKER)
            .collect()
    };
    let before = without_markers();
    let again = world.synced(&ids, 3);
    assert!(again.iter().all(|report| !report.changed), "{again:?}");
    assert_eq!(without_markers(), before);
}
