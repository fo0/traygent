//! What the settings rule refuses, skips and leaves alone, and the merge itself.
use super::*;
use crate::profiles::merge::{loses, split, Merge};
use crate::profiles::node::Node;
use crate::profiles::sync::refuses_sign_in;

const UNREADABLE: &str = "settings.json: ist kein lesbares JSON-Objekt und bleibt unverändert";

fn done(world: &World, id: &str) -> bool {
    let marker = read_marker(&world.dir(id)).unwrap_or_default();
    marker.done.iter().any(|rule| rule == FILE)
}

#[test]
fn unreadable_settings_stop_the_rule_and_are_never_overwritten() {
    for (name, broken) in [("json", "{ not json"), ("array", "[1]"), ("text", "\"x\"")] {
        for broken_id in ["default", "team"] {
            let world = World::new(&format!("settings-broken-{name}-{broken_id}"));
            let good_id = if broken_id == "team" {
                "default"
            } else {
                "team"
            };
            write(&settings(&world, broken_id), broken);
            write(&settings(&world, good_id), r#"{"theme":"dark"}"#);
            own(&world, "team");
            let reports = world.sync(&["team"], 1);
            assert_eq!(reports[0].errors, [UNREADABLE], "{name} {broken_id}");
            assert_eq!(read(&settings(&world, broken_id)), broken);
            assert_eq!(read(&settings(&world, good_id)), r#"{"theme":"dark"}"#);
            assert!(!done(&world, "team"));
            assert!(!base(&world, "team").exists());
        }
    }
    // Over the size cap: left alone as well.
    let world = World::new("settings-large");
    let large = format!(r#"{{"a":"{}"}}"#, "x".repeat(8 * 1024 * 1024));
    write(&settings(&world, "default"), &large);
    let reports = world.sync(&["team"], 1);
    assert_eq!(reports[0].errors, ["settings.json: ist zu groß"]);
    assert!(!settings(&world, "team").exists());
}

#[test]
fn an_empty_file_is_skipped_and_a_missing_one_is_created_from_the_shared_part() {
    let world = World::new("settings-empty");
    let ours = r#"{"theme":"dark","forceLoginOrgUUID":"org-default-synthetic","env":{"AWS_PROFILE":"aws-default-synthetic","EDITOR":"vim"}}"#;
    write(&settings(&world, "default"), ours);
    // A file the CLI is writing right now: no error, nothing written, not a first sync yet.
    write(&settings(&world, "team"), "");
    own(&world, "team");
    world.synced(&["team"], 1);
    assert_eq!(read(&settings(&world, "team")), "");
    assert_eq!(read(&settings(&world, "default")), ours);
    assert!(!done(&world, "team") && !base(&world, "team").exists());
    // Gone: created with the shared part only.
    std::fs::remove_file(settings(&world, "team")).unwrap();
    world.synced(&["team"], 2);
    let shared = json!({"theme": "dark", "env": {"EDITOR": "vim"}});
    assert_eq!(parsed(&settings(&world, "team")), shared);
    assert_eq!(parsed(&base(&world, "team")), shared);
    assert_eq!(read(&settings(&world, "default")), ours);
    assert!(done(&world, "team"));
    // A file deleted later has removed nothing: it comes back.
    std::fs::remove_file(settings(&world, "team")).unwrap();
    world.synced(&["team"], 3);
    assert_eq!(parsed(&settings(&world, "team")), shared);
    assert_eq!(read(&settings(&world, "default")), ours);
}

#[cfg(unix)]
#[test]
fn a_link_is_never_written_through_and_a_linked_base_directory_is_not_used() {
    use std::os::unix::fs::symlink;
    let world = World::new("settings-link");
    let elsewhere = world.home().join("dotfiles");
    write(&elsewhere.join("settings.json"), r#"{"theme":"light"}"#);
    write(&settings(&world, "default"), r#"{"theme":"dark"}"#);
    own(&world, "team");
    symlink(elsewhere.join("settings.json"), settings(&world, "team")).unwrap();
    let reports = world.sync(&["team"], 1);
    assert_eq!(
        reports[0].errors,
        ["settings.json: ist eine Verknüpfung und wird nicht überschrieben"]
    );
    assert_eq!(
        read(&elsewhere.join("settings.json")),
        r#"{"theme":"light"}"#
    );
    assert!(!world.dir("team").join(".traygent-backup").exists());

    // A base directory that is a link could lead into another home.
    let linked = World::new("settings-base-link");
    let other = linked.home().join("other");
    std::fs::create_dir_all(&other).unwrap();
    write(&settings(&linked, "default"), r#"{"theme":"dark"}"#);
    own(&linked, "team");
    symlink(&other, linked.dir("team").join(BASE_DIR)).unwrap();
    let reports = linked.sync(&["team"], 1);
    assert_eq!(
        reports[0].errors,
        ["settings.json: Abgleichsstand konnte nicht gespeichert werden"]
    );
    assert!(std::fs::read_dir(&other).unwrap().next().is_none());
    assert!(!done(&linked, "team"));
}

#[test]
fn the_base_directory_is_traygents_own_and_holds_the_shared_part_only() {
    let world = World::new("settings-base");
    let default = world.dir("default");
    // Not "foreign content": a directory holding nothing but the base stays the agent's.
    write(&base(&world, "left"), "{}");
    assert!(!refuses_sign_in(
        CLAUDE,
        &world.dir("left"),
        &default,
        &Default::default()
    ));
    write(&world.dir("left").join("notes.md"), "someone else's");
    assert!(refuses_sign_in(
        CLAUDE,
        &world.dir("left"),
        &default,
        &Default::default()
    ));
    // The base lives in profile directories only: never in the default home, a shared
    // directory or a backup.
    write(&settings(&world, "default"), r#"{"theme":"dark"}"#);
    write(
        &settings(&world, "team"),
        r#"{"theme":"light","apiKeyHelper":"helper-team-synthetic"}"#,
    );
    own(&world, "team");
    world.synced(&["team", "solo"], 1);
    let bases: Vec<PathBuf> = tree(world.home())
        .into_iter()
        .map(|(path, _, _)| path)
        .filter(|path| path.components().any(|part| part.as_os_str() == BASE_DIR))
        .collect();
    assert_eq!(
        bases,
        [
            base(&world, "left"),
            base(&world, "solo"),
            base(&world, "team")
        ]
    );
    assert_eq!(parsed(&base(&world, "team")), json!({"theme": "dark"}));
}

fn object(text: &str) -> Vec<(String, Node)> {
    Node::parse_object(text.as_bytes()).unwrap()
}

#[test]
fn the_merge_follows_the_side_that_changed() {
    let rule = recipe(CLAUDE).unwrap().settings.as_ref().unwrap();
    let merge = |base: Option<&str>, a: &str, b: &str, prefer_a: bool| {
        let mut merge = Merge::new(rule.set_paths, prefer_a);
        let base = base.map(object);
        let merged = merge.objects(base.as_deref(), &object(a), &object(b));
        let text = String::from_utf8(crate::profiles::node::to_file(&merged)).unwrap();
        (
            serde_json::from_str::<Value>(&text).unwrap(),
            merge.conflicts,
        )
    };
    let base = r#"{"a":1,"n":{"x":1,"y":2},"hooks":["h1"],"autoMode":{"allow":["p"]},"gone":true}"#;
    // One side changed each value: no conflict, whoever is preferred.
    let a =
        r#"{"a":2,"n":{"x":1,"y":2},"hooks":["h1"],"autoMode":{"allow":["p","q"]},"gone":true}"#;
    let b = r#"{"a":1,"n":{"x":1,"y":3,"z":4},"hooks":["h1","h2"],"autoMode":{"allow":["p"]}}"#;
    let expected = json!({
        "a": 2, "n": {"x": 1, "y": 3, "z": 4}, "hooks": ["h1", "h2"],
        "autoMode": {"allow": ["p", "q"]}
    });
    for prefer_a in [true, false] {
        assert_eq!(
            merge(Some(base), a, b, prefer_a),
            (expected.clone(), Vec::new())
        );
        assert_eq!(
            merge(Some(base), b, a, prefer_a),
            (expected.clone(), Vec::new())
        );
    }
    // Both changed: objects merge key by key, a list that is no set is one value, and a
    // key one side removed while the other changed it stays.
    let a =
        r#"{"a":1,"n":{"x":5,"y":2},"hooks":["h1","a"],"autoMode":{"allow":["p"]},"gone":false}"#;
    let b = r#"{"a":1,"n":{"x":6,"y":7},"hooks":["h1","b"],"autoMode":{"allow":["p"]}}"#;
    let (merged, conflicts) = merge(Some(base), a, b, false);
    assert_eq!(
        merged,
        json!({
            "a": 1, "n": {"x": 6, "y": 7}, "hooks": ["h1", "b"],
            "autoMode": {"allow": ["p"]}, "gone": false
        })
    );
    assert_eq!(conflicts, ["n.x", "hooks"]);
    assert_eq!(
        merge(Some(base), a, b, true).0["n"],
        json!({"x": 5, "y": 7})
    );
    // Without a base nothing is removed and sets are united, duplicates dropped.
    let (merged, conflicts) = merge(
        None,
        r#"{"permissions":{"deny":["x","y","x"]},"only":1}"#,
        r#"{"permissions":{"deny":["z","y"]},"other":2}"#,
        true,
    );
    assert_eq!(
        merged,
        json!({"permissions": {"deny": ["x", "y", "z"]}, "only": 1, "other": 2})
    );
    assert!(conflicts.is_empty());
}

#[test]
fn the_recipe_names_what_belongs_to_the_sign_in() {
    let rule = recipe(CLAUDE).unwrap().settings.as_ref().unwrap();
    for name in [
        "ANTHROPIC_API_KEY",
        "anthropic_base_url",
        "CLAUDE_CONFIG_DIR",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_USE_BEDROCK",
        "AWS_REGION",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "AZURE_TENANT_ID",
    ] {
        assert!(rule.own_env(name), "{name}");
    }
    for name in [
        "EDITOR",
        "CLAUDE_CONFIG_DIR_2",
        "CLAUDE_CODE_MAX_OUTPUT_TOKENS",
        "MY_AWS_KEY",
    ] {
        assert!(!rule.own_env(name), "{name}");
    }
    let doc = object(
        r#"{"apiKeyHelper":"h","forceLoginOrgUUID":"o","forceLoginMethod":"m","awsAuthRefresh":"r",
            "awsCredentialExport":"e","anthropicProfile":"p","anthropicProfiles":{},
            "env":{"AWS_PROFILE":"x"},"theme":"dark"}"#,
    );
    let parts = split(&doc, rule);
    // `env` is shared only while it holds a shared variable.
    assert_eq!(parts.shared, object(r#"{"theme":"dark"}"#));
    assert_eq!(parts.own.len(), 7);
    assert_eq!(parts.own_env, object(r#"{"AWS_PROFILE":"x"}"#));
    // A non-object `env` is an ordinary shared value.
    assert_eq!(split(&object(r#"{"env":"x"}"#), rule).shared.len(), 1);
    let node = |text: &str| Node::Object(object(text));
    assert!(loses(&node(r#"{"a":1,"b":2}"#), &node(r#"{"a":1}"#)));
    assert!(loses(
        &node(r#"{"l":["x","y"]}"#),
        &node(r#"{"l":["y","z"]}"#)
    ));
    assert!(loses(&node(r#"{"o":{"k":1}}"#), &node(r#"{"o":"text"}"#)));
    assert!(!loses(
        &node(r#"{"a":1,"l":["x"]}"#),
        &node(r#"{"a":2,"l":["x","y"],"n":0}"#)
    ));
}
