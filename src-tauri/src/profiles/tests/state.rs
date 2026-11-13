//! The state file's shared keys: united entry by entry, everything else left alone.
use super::*;
use crate::profiles::json::{merge_keys, Base, NotJson, Shared, MAX_BASE_NAMES};
use serde_json::{json, Value};

fn parsed(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap()
}

fn text(bytes: Option<Vec<u8>>) -> Option<String> {
    bytes.map(|bytes| String::from_utf8(bytes).unwrap())
}

#[test]
fn shared_keys_are_united_entry_by_entry() {
    let keys = Shared {
        entries: &["mcpServers", "projects"],
        ..Shared::default()
    };
    let merge = |ours: &str, theirs: &str, default_wins: bool| {
        let merged = merge_keys(
            Some(ours.as_bytes()),
            Some(theirs.as_bytes()),
            &keys,
            default_wins,
            None,
        )
        .unwrap();
        // Without a base nothing is ever removed.
        assert!(!merged.removed_default && !merged.removed_profile);
        let (ours, theirs) = (merged.default, merged.profile);
        (text(ours), text(theirs))
    };
    // Each side has an entry the other lacks; `both` differs. Nothing is lost on either
    // side, whichever file is newer — only the entry both have follows the winner.
    let ours =
        r#"{"id":"default","projects":{"C:/a":{"n":1},"C:/both":{"n":1}},"mcpServers":{"docs":1}}"#;
    let theirs =
        r#"{"mcpServers":{"tickets":2},"id":"team","projects":{"C:/both":{"n":2},"C:/b":{"n":3}}}"#;
    let (new_ours, new_theirs) = merge(ours, theirs, true);
    assert_eq!(
        new_ours.unwrap(),
        r#"{"id":"default","projects":{"C:/a":{"n":1},"C:/both":{"n":1},"C:/b":{"n":3}},"mcpServers":{"docs":1,"tickets":2}}"#
    );
    assert_eq!(
        new_theirs.unwrap(),
        r#"{"mcpServers":{"tickets":2,"docs":1},"id":"team","projects":{"C:/both":{"n":1},"C:/b":{"n":3},"C:/a":{"n":1}}}"#
    );
    let (new_ours, new_theirs) = merge(ours, theirs, false);
    assert!(new_ours.unwrap().contains(r#""C:/both":{"n":2}"#));
    assert!(new_theirs.unwrap().contains(r#""C:/both":{"n":2}"#));
    // The same values in another layout are no change, on either side.
    let same = merge(
        "{\"mcpServers\": {\"a\": 1,\n \"b\": [1, 2]}}",
        r#"{"mcpServers":{"b":[1,2],"a":1},"id":"keep"}"#,
        false,
    );
    assert_eq!(same, (None, None));
    assert_eq!(merge("{}", "{}", true), (None, None));
    // A key one side lacks arrives whole; a value that is no object counts as one entry.
    let (new_ours, new_theirs) = merge(
        r#"{"mcpServers":{"a":1}}"#,
        r#"{"projects":[1],"x":true}"#,
        true,
    );
    assert_eq!(
        new_ours.unwrap(),
        r#"{"mcpServers":{"a":1},"projects":[1]}"#
    );
    assert_eq!(
        new_theirs.unwrap(),
        r#"{"projects":[1],"x":true,"mcpServers":{"a":1}}"#
    );
    assert_eq!(
        merge(r#"{"projects":[1]}"#, r#"{"projects":{"p":1}}"#, true)
            .1
            .unwrap(),
        r#"{"projects":[1]}"#
    );
    assert_eq!(
        merge(r#"{"projects":[1]}"#, r#"{"projects":{"p":1}}"#, true).0,
        None
    );
    // A profile without a state file gets one with the shared keys only.
    let source = br#"{"mcpServers":{"a":1},"oauthAccount":{"emailAddress":"max@example.test"}}"#;
    let created = merge_keys(Some(source), None, &keys, true, None).unwrap();
    assert_eq!(created.default, None);
    assert_eq!(
        text(created.profile).unwrap(),
        "{\n  \"mcpServers\": {\"a\":1}\n}"
    );
    assert_eq!(created.base["mcpServers"], ["a"]);
    assert_eq!(
        merge_keys(None, None, &keys, true, None),
        Ok(Default::default())
    );
    // A pretty file keeps its indentation for the entries it gains.
    let pretty = "{\n  \"mcpServers\": {\n    \"a\": 1\n  }\n}\n";
    let (grown, _) = merge(pretty, r#"{"mcpServers":{"b":{"x":1}}}"#, true);
    assert_eq!(
        grown.unwrap(),
        "{\n  \"mcpServers\": {\n    \"a\": 1,\n    \"b\": {\"x\":1}\n  }\n}\n"
    );
    // Anything that is no JSON object stops the merge.
    for broken in ["", "{ nope", "[1]", "null", "{\"a\":1} trailing"] {
        let broken = Some(broken.as_bytes());
        let fine: Option<&[u8]> = Some(b"{\"mcpServers\":{}}");
        assert_eq!(merge_keys(broken, fine, &keys, true, None), Err(NotJson));
        assert_eq!(merge_keys(fine, broken, &keys, true, None), Err(NotJson));
    }
}

#[test]
fn only_the_shared_keys_of_the_state_file_travel() {
    let world = World::new("profiles-state");
    write(&world.state("default"), DEFAULT_STATE);
    write(&world.state("team"), TEAM_STATE);
    // First sync: the default home's entries win, even though the profile's file is newer.
    touch(&world.state("default"), 10);
    touch(&world.state("team"), 500);
    world.synced(&["team"], 1);
    assert_eq!(
        read(&world.state("default")),
        DEFAULT_STATE,
        "it gains nothing: not written"
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
    let keys = [
        "installMethod",
        "oauthAccount",
        "mcpServers",
        "bigNumber",
        "numStartups",
        "projects",
    ];
    let order: Vec<usize> = keys
        .iter()
        .map(|key| team.find(&format!("\n  \"{key}\":")).unwrap())
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{team}");
    assert!(team.ends_with("}\n"));
    // What the first sync rewrote is in the profile's own backup, identity and all — and
    // nowhere in the default home.
    assert_eq!(read(&world.backup("team").join(".claude.json")), TEAM_STATE);
    assert!(!world
        .dir("default")
        .join(super::super::files::BACKUP_DIR)
        .exists());

    // Later both sides gain entries; the CLI rewrote the default home's file last, for an
    // unrelated key. Nothing is lost on either side.
    let mut edited: Value = serde_json::from_str(&team).unwrap();
    edited["mcpServers"]["tickets"] = json!({ "command": "tickets-mcp" });
    edited["projects"]["C:/Code/demo"]["hasTrustDialogAccepted"] = json!(false);
    write(
        &world.state("team"),
        &serde_json::to_string_pretty(&edited).unwrap(),
    );
    let mut ours: Value = serde_json::from_str(DEFAULT_STATE).unwrap();
    ours["projects"]["C:/Code/other"] = json!({ "hasTrustDialogAccepted": true });
    ours["numStartups"] = json!(6);
    write(
        &world.state("default"),
        &serde_json::to_string_pretty(&ours).unwrap(),
    );
    touch(&world.state("team"), 600);
    touch(&world.state("default"), 700);
    world.synced(&["team"], 2);
    for id in ["default", "team"] {
        let value = parsed(&world.state(id));
        assert_eq!(
            value["mcpServers"]["tickets"]["command"], "tickets-mcp",
            "{id}"
        );
        assert_eq!(value["mcpServers"]["docs"]["command"], "docs-mcp", "{id}");
        assert_eq!(
            value["projects"]["C:/Code/other"]["hasTrustDialogAccepted"], true,
            "{id}"
        );
        // The entry both have follows the newer file.
        assert_eq!(
            value["projects"]["C:/Code/demo"]["hasTrustDialogAccepted"], true,
            "{id}"
        );
    }
    let ours = parsed(&world.state("default"));
    assert_eq!(ours["oauthAccount"]["emailAddress"], "max@example.test");
    assert_eq!(ours["numStartups"], 6);
    assert_eq!(
        parsed(&world.state("team"))["oauthAccount"]["emailAddress"],
        "team@example.test"
    );
    // A third run has nothing to do.
    assert!(!world.synced(&["team"], 3)[0].changed);
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

fn base(servers: &[&str]) -> Base {
    let names = servers.iter().map(|name| name.to_string()).collect();
    Base::from([("mcpServers".to_string(), names)])
}

/// With the names of the last sync an entry one side lacks is a removal when the base
/// knows it, an addition when it does not.
#[test]
fn a_base_tells_a_removed_entry_from_an_added_one() {
    let keys = Shared {
        entries: &["mcpServers"],
        ..Shared::default()
    };
    let merge = |ours: &str, theirs: &str, known: Option<&Base>| {
        merge_keys(
            Some(ours.as_bytes()),
            Some(theirs.as_bytes()),
            &keys,
            true,
            known,
        )
        .unwrap()
    };
    let known = base(&["docs", "old", "tickets"]);
    // Removed in the default home (`old`), removed in the profile (`tickets`), added on
    // each side (`new-d`, `new-p`), kept on both (`docs`).
    let ours = r#"{"id":"default","mcpServers":{"docs":1,"tickets":2,"new-d":4}}"#;
    let theirs = r#"{"mcpServers":{"docs":1,"old":3,"new-p":5},"id":"team"}"#;
    let merged = merge(ours, theirs, Some(&known));
    assert_eq!(
        text(merged.default.clone()).unwrap(),
        r#"{"id":"default","mcpServers":{"docs":1,"new-d":4,"new-p":5}}"#
    );
    assert_eq!(
        text(merged.profile.clone()).unwrap(),
        r#"{"mcpServers":{"docs":1,"new-p":5,"new-d":4},"id":"team"}"#
    );
    assert!(merged.removed_default && merged.removed_profile);
    assert_eq!(merged.base["mcpServers"], ["docs", "new-d", "new-p"]);
    // The removal wins even when the other side changed the entry since.
    let changed = merge(
        r#"{"mcpServers":{"docs":1}}"#,
        r#"{"mcpServers":{"docs":1,"old":99}}"#,
        Some(&known),
    );
    assert_eq!(
        text(changed.profile).unwrap(),
        r#"{"mcpServers":{"docs":1}}"#
    );
    assert_eq!(changed.default, None);
    // Removing the only entry leaves an empty object, not a missing key.
    let only = base(&["old"]);
    let last = merge(
        r#"{"mcpServers":{}}"#,
        r#"{"mcpServers":{"old":3}}"#,
        Some(&only),
    );
    assert_eq!(text(last.profile).unwrap(), r#"{"mcpServers":{}}"#);
    assert_eq!(last.base["mcpServers"], Vec::<String>::new());
    // The same input without a base, or with one from another key, is a plain union.
    for none in [None, Some(&Base::new())] {
        let united = merge(ours, theirs, none);
        assert!(!united.removed_default && !united.removed_profile);
        let value: Value = serde_json::from_slice(&united.default.unwrap()).unwrap();
        assert_eq!(
            value["mcpServers"],
            json!({"docs":1,"tickets":2,"new-d":4,"old":3,"new-p":5})
        );
        assert_eq!(
            united.base["mcpServers"],
            ["docs", "new-d", "new-p", "old", "tickets"]
        );
    }
    // A missing file or a missing key has removed nothing.
    let source = r#"{"mcpServers":{"docs":1,"old":3}}"#;
    let created = merge_keys(Some(source.as_bytes()), None, &keys, true, Some(&known)).unwrap();
    assert!(text(created.profile).unwrap().contains("old"));
    assert_eq!(created.default, None);
    let keyless = merge(source, r#"{"id":"team"}"#, Some(&known));
    assert_eq!(keyless.default, None);
    assert!(text(keyless.profile).unwrap().contains("old") && !keyless.removed_default);
    // A key with more entries than the cap keeps no base: the next sync only unites.
    let many: Vec<String> = (0..=MAX_BASE_NAMES)
        .map(|n| format!("\"s{n}\":1"))
        .collect();
    let big = format!("{{\"mcpServers\":{{{}}}}}", many.join(","));
    assert!(!merge(&big, &big, None).base.contains_key("mcpServers"));
}

/// Through the files: a removal travels in either direction, an addition still does, and
/// neither a first sync nor a marker from before the base ever removes anything.
#[test]
fn a_removed_entry_stays_removed_in_every_profile() {
    let world = World::new("profiles-removal");
    let state = |servers: &[&str], id: &str| {
        let servers: serde_json::Map<String, Value> = servers
            .iter()
            .map(|name| (name.to_string(), json!({ "command": name })))
            .collect();
        serde_json::to_string_pretty(&json!({ "id": id, "mcpServers": servers, "projects": {} }))
            .unwrap()
    };
    let servers = |id: &str| -> Vec<String> {
        let value = parsed(&world.state(id));
        let mut names: Vec<_> = value["mcpServers"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        names.sort();
        names
    };
    // First sync: each side has an entry the other lacks — united, nothing removed.
    write(&world.state("default"), &state(&["docs", "old"], "default"));
    write(&world.state("team"), &state(&["tickets"], "team"));
    world.synced(&["team", "solo"], 1);
    for id in ["default", "team", "solo"] {
        assert_eq!(servers(id), ["docs", "old", "tickets"], "{id}");
    }
    let marker = read_marker(&world.dir("team")).unwrap();
    assert_eq!(
        marker.base.unwrap()["mcpServers"],
        ["docs", "old", "tickets"]
    );
    assert!(
        !read(&world.dir("team").join(MARKER)).contains("command"),
        "names only"
    );
    // `claude mcp remove old` in the default home, `… remove tickets` in the profile, and
    // a server added in each.
    write(
        &world.state("default"),
        &state(&["docs", "tickets", "new-d"], "default"),
    );
    write(
        &world.state("team"),
        &state(&["docs", "old", "new-p"], "team"),
    );
    world.synced(&["team", "solo"], 2);
    for id in ["default", "team", "solo"] {
        assert_eq!(servers(id), ["docs", "new-d", "new-p"], "{id}");
    }
    for id in ["default", "team"] {
        assert_eq!(parsed(&world.state(id))["id"], id, "identity stays");
    }
    // A file that lost entries is in its own home's backup as it was.
    let kept = world
        .dir("default")
        .join(super::super::files::BACKUP_DIR)
        .join("20261113T090200Z/.claude.json");
    assert!(read(&kept).contains("tickets"));
    // Nothing more to do; the removed entries do not come back.
    assert!(world
        .synced(&["team", "solo"], 3)
        .iter()
        .all(|report| !report.changed));
    // A marker from before the base: the next sync unites and never removes.
    write(
        &world.dir("team").join(MARKER),
        r#"{"version":1,"syncedAt":"2026-11-13T09:00:00Z","done":[".claude.json"]}"#,
    );
    write(&world.state("team"), &state(&["docs"], "team"));
    world.synced(&["team"], 4);
    assert_eq!(servers("team"), ["docs", "new-d", "new-p"]);
    assert_eq!(servers("default"), ["docs", "new-d", "new-p"]);
    assert!(
        read_marker(&world.dir("team")).unwrap().base.is_some(),
        "recorded from now on"
    );
}

/// A state file that was recreated holds none of the names the base knows: that is a
/// reset, not a row of removals, and nothing is removed anywhere.
#[test]
fn a_reset_side_removes_nothing_but_a_partial_removal_still_travels() {
    let keys = Shared {
        entries: &["mcpServers"],
        ..Shared::default()
    };
    let merge = |ours: &str, theirs: &str, known: &Base| {
        merge_keys(
            Some(ours.as_bytes()),
            Some(theirs.as_bytes()),
            &keys,
            true,
            Some(known),
        )
        .unwrap()
    };
    let known = base(&["a", "b", "c"]);
    let full = r#"{"mcpServers":{"a":1,"b":2,"c":3}}"#;
    // Either side wiped, with or without something new in it: united.
    for wiped in [r#"{"mcpServers":{}}"#, r#"{"mcpServers":{"new":9}}"#] {
        for (ours, theirs) in [(full, wiped), (wiped, full)] {
            let merged = merge(ours, theirs, &known);
            assert!(
                !merged.removed_default && !merged.removed_profile,
                "{wiped}"
            );
            for side in [merged.default, merged.profile].into_iter().flatten() {
                let value: Value = serde_json::from_slice(&side).unwrap();
                for name in ["a", "b", "c"] {
                    assert!(value["mcpServers"].get(name).is_some(), "{name} in {value}");
                }
            }
            assert!(
                merged.base["mcpServers"].len() >= 3,
                "the base is recorded as usual"
            );
        }
    }
    // One of three removed: still a removal, in either direction.
    let two = r#"{"mcpServers":{"a":1,"c":3}}"#;
    let merged = merge(two, full, &known);
    assert_eq!(text(merged.profile).unwrap(), two);
    assert!(merged.removed_profile && merged.default.is_none());
    let merged = merge(full, two, &known);
    assert_eq!(text(merged.default).unwrap(), two);
    assert_eq!(merged.base["mcpServers"], ["a", "c"]);
    // Two of three removed too — one known name is left on that side.
    let one = r#"{"mcpServers":{"b":2}}"#;
    assert_eq!(text(merge(full, one, &known).default).unwrap(), one);

    // Through the files: the profile's state file is recreated with empty `projects`.
    let world = World::new("profiles-reset");
    let projects = json!({ "C:/a": { "trust": true }, "C:/b": { "trust": true }, "C:/c": {} });
    let state = |projects: &Value| {
        serde_json::to_string_pretty(&json!({ "projects": projects, "mcpServers": { "docs": 1 } }))
            .unwrap()
    };
    write(&world.state("default"), &state(&projects));
    world.synced(&["team"], 1);
    assert_eq!(parsed(&world.state("team"))["projects"], projects);
    write(
        &world.state("team"),
        r#"{"projects":{},"mcpServers":{"docs":1}}"#,
    );
    touch(&world.state("default"), 10);
    touch(&world.state("team"), 900);
    world.synced(&["team"], 2);
    assert_eq!(
        parsed(&world.state("default"))["projects"],
        projects,
        "nothing removed"
    );
    assert_eq!(
        parsed(&world.state("team"))["projects"],
        projects,
        "and the profile has them back"
    );
    // Dropping one project's entry afterwards still reaches the default home.
    let mut fewer = projects.clone();
    fewer.as_object_mut().unwrap().remove("C:/b");
    write(&world.state("team"), &state(&fewer));
    world.synced(&["team"], 3);
    assert_eq!(parsed(&world.state("default"))["projects"], fewer);
}
