//! Review fixes of the settings rule (#35): the reset guard, a base for a new profile, a
//! shared `env` that is no object, byte order marks, number tokens, a write meanwhile.
use super::*;
use crate::profiles::node::{to_file, Node};
use crate::profiles::pair::AFTER_READ;

fn object(text: &str) -> Node {
    Node::Object(Node::parse_object(text.as_bytes()).unwrap())
}

fn entries(text: &str) -> Vec<(String, Node)> {
    Node::parse_object(text.as_bytes()).unwrap()
}

/// A settings file emptied in one profile is a reset, not the removal of every key: the
/// shared settings of the other homes stay, and the emptied file gets them back.
#[test]
fn an_emptied_settings_file_does_not_empty_every_profile() {
    let world = World::new("settings-reset");
    let ids = ["team", "solo"];
    let shared = json!({"theme": "dark", "model": "opus", "permissions": {"allow": ["A"]}});
    put(&world, "default", shared.clone(), 10);
    world.synced(&ids, 1);
    put(&world, "team", json!({}), 500);
    world.synced(&ids, 2);
    for id in ["default", "team", "solo"] {
        assert_eq!(parsed(&settings(&world, id)), shared, "{id}");
    }
    assert_eq!(parsed(&base(&world, "team")), shared);
    // The default home emptied: the profiles hand everything back.
    put(
        &world,
        "default",
        json!({"forceLoginMethod": "console"}),
        900,
    );
    world.synced(&ids, 3);
    let mut with_own = shared.clone();
    with_own["forceLoginMethod"] = json!("console");
    assert_eq!(parsed(&settings(&world, "default")), with_own);
    for id in ids {
        assert_eq!(parsed(&settings(&world, id)), shared, "{id}");
    }
    // Removing some keys, or the only one, still is a removal.
    put(&world, "team", json!({"theme": "dark"}), 950);
    world.synced(&ids, 4);
    for id in ["default", "solo"] {
        assert_eq!(parsed(&settings(&world, id))["model"], Value::Null, "{id}");
        assert_eq!(parsed(&settings(&world, id))["theme"], "dark", "{id}");
    }
    put(&world, "solo", json!({}), 990);
    world.synced(&ids, 5);
    for id in ["default", "team", "solo"] {
        assert_eq!(parsed(&settings(&world, id))["theme"], Value::Null, "{id}");
    }
}

/// A profile added later has no base; another profile's base tells what the default home
/// removed in the meantime, so the new profile's old file does not bring it back.
#[test]
fn a_new_profile_does_not_bring_back_what_the_default_home_removed() {
    let world = World::new("settings-seed");
    let old = json!({
        "old": 1, "theme": "dark", "permissions": {"allow": ["A", "Gone"]},
        "nested": {"keep": 1, "drop": 2}
    });
    put(&world, "default", old.clone(), 10);
    world.synced(&["team"], 1);
    // The default home removes a key, a list item and a nested key and changes a value;
    // the new profile's file still is the old state plus a key of its own.
    let now = json!({
        "theme": "light", "permissions": {"allow": ["A"]}, "nested": {"keep": 1}
    });
    put(&world, "default", now.clone(), 100);
    let mut theirs = old.clone();
    theirs["mine"] = json!(true);
    theirs["permissions"]["allow"] = json!(["A", "Gone", "New"]);
    put(&world, "solo", theirs.clone(), 900);
    own(&world, "solo");
    world.synced(&["team", "solo"], 2);
    let merged = json!({
        "theme": "light", "permissions": {"allow": ["A", "New"]}, "nested": {"keep": 1},
        "mine": true
    });
    for id in ["default", "team", "solo"] {
        assert_eq!(parsed(&settings(&world, id)), merged, "{id}");
    }
    assert_eq!(parsed(&base(&world, "solo")), merged);
    // The first sync keeps what the new profile had.
    assert_eq!(parsed(&world.backup("solo").join(FILE)), theirs);
    // Without any base (the first profile ever) everything is united as before.
    let alone = World::new("settings-seed-none");
    put(&alone, "default", now, 100);
    put(&alone, "solo", old, 900);
    own(&alone, "solo");
    alone.synced(&["solo"], 1);
    assert_eq!(parsed(&settings(&alone, "default"))["old"], 1);
}

#[test]
fn a_shared_env_that_is_no_object_stops_the_rule_while_a_side_has_own_variables() {
    let world = World::new("settings-env");
    let ours = r#"{"env":"broken","theme":"dark"}"#;
    let theirs = r#"{"env":{"ANTHROPIC_API_KEY":"key-team-synthetic","EDITOR":"vim"}}"#;
    write(&settings(&world, "default"), ours);
    write(&settings(&world, "team"), theirs);
    own(&world, "team");
    for minute in [1, 2] {
        let reports = world.sync(&["team"], minute);
        assert_eq!(
            reports[0].errors,
            ["settings.json: „env“ ist kein Objekt und bleibt unverändert"]
        );
        assert_eq!(read(&settings(&world, "default")), ours);
        assert_eq!(read(&settings(&world, "team")), theirs);
        assert!(!base(&world, "team").exists());
        assert!(!world.dir("team").join(".traygent-backup").exists());
    }
    // Without own variables it is an ordinary value.
    write(&settings(&world, "team"), r#"{"env":{"EDITOR":"vim"}}"#);
    world.synced(&["team"], 3);
    assert_eq!(parsed(&settings(&world, "team"))["env"], "broken");
}

#[test]
fn a_byte_order_mark_does_not_block_the_settings_or_the_state_file() {
    let world = World::new("settings-bom");
    let bom = "\u{feff}";
    write(
        &settings(&world, "default"),
        &format!("{bom}{{\"theme\":\"dark\"}}"),
    );
    write(
        &settings(&world, "team"),
        &format!("{bom}{{\"model\":\"opus\"}}"),
    );
    write(
        &world.state("default"),
        &format!("{bom}{{\n  \"oauthAccount\": {{\"emailAddress\": \"max@example.test\"}},\n  \"mcpServers\": {{\"docs\": {{}}}}\n}}\n"),
    );
    write(
        &world.state("team"),
        &format!("{bom}{{\"mcpServers\":{{\"team\":{{}}}}}}"),
    );
    world.synced(&["team"], 1);
    let merged = json!({"theme": "dark", "model": "opus"});
    for id in ["default", "team"] {
        assert_eq!(parsed(&settings(&world, id)), merged, "{id}");
        // The state file keeps its mark and its other keys; the merge went through.
        let state = read(&world.state(id));
        assert!(state.starts_with(bom), "{id}");
        let value: Value = serde_json::from_str(state.trim_start_matches(bom)).unwrap();
        assert_eq!(value["mcpServers"], json!({"docs": {}, "team": {}}), "{id}");
    }
    assert!(read(&world.state("default")).contains("max@example.test"));
    // A file of nothing but the mark is one being written.
    write(&settings(&world, "team"), bom);
    world.synced(&["team"], 2);
    assert_eq!(read(&settings(&world, "team")), bom);
}

#[test]
fn number_tokens_survive_a_rewrite_untouched() {
    let world = World::new("settings-numbers");
    let ours = r#"{"ratio":0.1234567890123456789012345,"big":123456789012345678901234567890,"exp":1.50e+3,"n":1}"#;
    write(&settings(&world, "default"), ours);
    write(
        &settings(&world, "team"),
        r#"{"n":1.0,"extra":-0.30000000000000004}"#,
    );
    own(&world, "team");
    world.synced(&["team"], 1);
    for id in ["default", "team"] {
        let text = read(&settings(&world, id));
        for token in [
            "\"ratio\": 0.1234567890123456789012345",
            "\"big\": 123456789012345678901234567890",
            "\"exp\": 1.50e+3",
            "\"extra\": -0.30000000000000004",
        ] {
            assert!(text.contains(token), "{token} in {id}: {text}");
        }
    }
    let kept = read(&base(&world, "team"));
    assert!(kept.contains("0.1234567890123456789012345") && kept.contains("1.50e+3"));
    // Two spellings of one number are no difference; 1 and 1.0 are, as for serde_json.
    assert_eq!(
        object(r#"{"a":1.50,"b":100.0}"#),
        object(r#"{"b":1e2,"a":1.5}"#)
    );
    assert_ne!(object(r#"{"a":1}"#), object(r#"{"a":1.0}"#));
    assert_ne!(object(r#"{"a":1}"#), object(r#"{"a":"1"}"#));
}

/// A session writes a side between the read and the write: neither file is replaced and
/// the base stays what it was, so the next sync merges the new content properly.
#[test]
fn a_side_written_meanwhile_leaves_both_files_and_the_base_alone() {
    let world = World::new("settings-meanwhile");
    put(&world, "default", json!({"theme": "dark"}), 10);
    world.synced(&["team"], 1);
    let before = read(&base(&world, "team"));
    put(
        &world,
        "team",
        json!({"theme": "dark", "model": "opus"}),
        200,
    );
    let file = settings(&world, "default");
    AFTER_READ.with(|hook| {
        let file = file.clone();
        *hook.borrow_mut() = Some(Box::new(move || {
            write(&file, r#"{"theme":"dark","verbose":true}"#);
        }));
    });
    let reports = world.sync(&["team"], 2);
    AFTER_READ.with(|hook| *hook.borrow_mut() = None);
    assert_eq!(
        reports[0].errors,
        ["settings.json: wurde gerade geändert, nächster Versuch beim nächsten Abgleich"]
    );
    assert_eq!(read(&file), r#"{"theme":"dark","verbose":true}"#);
    assert_eq!(parsed(&settings(&world, "team"))["model"], "opus");
    assert_eq!(read(&base(&world, "team")), before);
    assert!(!world.dir("default").join(".traygent-backup").exists());
    // The next sync merges both changes.
    world.synced(&["team"], 3);
    let merged = json!({"theme": "dark", "verbose": true, "model": "opus"});
    assert_eq!(parsed(&settings(&world, "default")), merged);
    assert_eq!(parsed(&settings(&world, "team")), merged);
}

#[test]
fn the_ordered_json_value_reads_and_prints_what_json_allows() {
    // Escapes survive; a duplicate key reads as its last occurrence, at its first place.
    let text =
        r#"{"a\"b\\c":"line\nbreak ä 😀 \/","dup":1,"x":null,"dup":[true,false,{}],"e":[],"o":{}}"#;
    let doc = entries(text);
    let keys: Vec<&str> = doc.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, ["a\"b\\c", "dup", "x", "e", "o"]);
    let printed = String::from_utf8(to_file(&doc)).unwrap();
    assert_eq!(
        printed,
        "{\n  \"a\\\"b\\\\c\": \"line\\nbreak ä 😀 /\",\n  \"dup\": [\n    true,\n    false,\n    {}\n  ],\n  \"x\": null,\n  \"e\": [],\n  \"o\": {}\n}\n"
    );
    assert_eq!(entries(&printed), doc);
    let value: Value = serde_json::from_str(&printed).unwrap();
    assert_eq!(value["a\"b\\c"], "line\nbreak ä 😀 /");
    // A byte order mark is skipped; anything that is no object, or no JSON, is refused.
    assert_eq!(
        Node::parse_object("\u{feff} {\"a\":1}".as_bytes()),
        Some(entries(r#"{"a":1}"#))
    );
    for bad in [
        "",
        "[]",
        "1",
        "\"x\"",
        "null",
        "{",
        "{\"a\":1}x",
        "{'a':1}",
        "{\"a\":01}",
        "\u{feff}",
    ] {
        assert_eq!(Node::parse_object(bad.as_bytes()), None, "{bad}");
    }
    // Nesting beyond what the parser allows is refused, not followed.
    let deep = |levels: usize| format!("{}1{}", "{\"a\":".repeat(levels), "}".repeat(levels));
    assert!(Node::parse_object(deep(100).as_bytes()).is_some());
    assert_eq!(Node::parse_object(deep(200).as_bytes()), None);
    assert_eq!(Node::parse_object(deep(100_000).as_bytes()), None);
    // Numbers keep their token.
    let numbers = r#"{"a":-0,"b":1E5,"c":18446744073709551616,"d":0.10}"#;
    let printed = String::from_utf8(to_file(&entries(numbers))).unwrap();
    assert_eq!(
        printed,
        "{\n  \"a\": -0,\n  \"b\": 1E5,\n  \"c\": 18446744073709551616,\n  \"d\": 0.10\n}\n"
    );
}

/// Another profile's base only tells what the default home removed of the things that
/// base knew: what a new profile holds beyond it is its own and stays.
#[test]
fn a_seed_removes_only_what_it_knows_and_nothing_after_a_reset() {
    let world = World::new("settings-seed-partial");
    put(
        &world,
        "default",
        json!({"theme": "dark", "permissions": {"allow": ["a"]}, "nested": {"x": 1, "y": 2}}),
        10,
    );
    world.synced(&["team"], 1);
    // The default home drops the whole `permissions` block, `nested.x` and changes nothing
    // else; the new profile holds more than the old block ever had.
    put(
        &world,
        "default",
        json!({"theme": "dark", "nested": {"y": 2}}),
        100,
    );
    let theirs = json!({
        "theme": "dark",
        "permissions": {"allow": ["a", "b", "c"], "deny": ["d"], "defaultMode": "plan"},
        "nested": {"x": 5, "y": 2, "z": 3}
    });
    put(&world, "solo", theirs.clone(), 900);
    own(&world, "solo");
    world.synced(&["team", "solo"], 2);
    let merged = json!({
        "theme": "dark",
        // `a` was the seed's and went with the block; `b`, `c`, `deny` and the mode are new.
        "permissions": {"allow": ["b", "c"], "deny": ["d"], "defaultMode": "plan"},
        // `x` differs from the seed's value: the profile changed it, it stays.
        "nested": {"y": 2, "x": 5, "z": 3}
    });
    for id in ["default", "team", "solo"] {
        assert_eq!(parsed(&settings(&world, id)), merged, "{id}");
    }
    assert_eq!(parsed(&world.backup("solo").join(FILE)), theirs);
    // A block the profile holds exactly as the seed knew it goes entirely.
    let exact = World::new("settings-seed-exact");
    let old = json!({"theme": "dark", "permissions": {"allow": ["a"]}, "old": 1});
    put(&exact, "default", old.clone(), 10);
    exact.synced(&["team"], 1);
    put(&exact, "default", json!({"theme": "dark"}), 100);
    put(&exact, "solo", old.clone(), 900);
    own(&exact, "solo");
    exact.synced(&["team", "solo"], 2);
    for id in ["default", "team", "solo"] {
        assert_eq!(
            parsed(&settings(&exact, id)),
            json!({"theme": "dark"}),
            "{id}"
        );
    }
    // The default home emptied (none of the seed's keys left): the seed tells nothing,
    // the new profile keeps everything and the default home gets it back.
    let reset = World::new("settings-seed-reset");
    put(&reset, "default", old.clone(), 10);
    reset.synced(&["team"], 1);
    put(&reset, "default", json!({}), 100);
    put(&reset, "solo", old.clone(), 900);
    own(&reset, "solo");
    reset.synced(&["solo", "team"], 2);
    for id in ["default", "team", "solo"] {
        assert_eq!(parsed(&settings(&reset, id)), old, "{id}");
    }
}
