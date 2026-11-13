//! The state file's shared keys: united entry by entry, everything else left alone.
use super::*;
use crate::profiles::json::{Base, Shared};
use crate::profiles::node::Node;
use crate::profiles::state::{merge_keys, Known, Merged, Refused};
use serde_json::{json, Value};

fn parsed(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap()
}

fn text(bytes: Option<Vec<u8>>) -> Option<String> {
    bytes.map(|bytes| String::from_utf8(bytes).unwrap())
}

fn keys() -> Shared<'static> {
    Shared {
        entries: &["mcpServers", "projects"],
        ..Shared::default()
    }
}

fn nodes(text: &str) -> Vec<(String, Node)> {
    Node::parse_object(text.as_bytes()).unwrap()
}

fn merge(ours: &str, theirs: &str, known: Known, default_newer: bool) -> Merged {
    let (ours, theirs) = (Some(ours.as_bytes()), Some(theirs.as_bytes()));
    merge_keys(ours, theirs, &keys(), known, default_newer).unwrap()
}

#[test]
fn shared_keys_are_united_entry_by_entry() {
    let texts = |merged: Merged| (text(merged.default), text(merged.profile));
    // Each side has an entry the other lacks; `both` differs. Nothing is lost on either
    // side — only the entry both have follows the winner: without a base the default home.
    let ours =
        r#"{"id":"default","projects":{"C:/a":{"n":1},"C:/both":{"n":1}},"mcpServers":{"docs":1}}"#;
    let theirs =
        r#"{"mcpServers":{"tickets":2},"id":"team","projects":{"C:/both":{"n":2},"C:/b":{"n":3}}}"#;
    for newer in [true, false] {
        let merged = merge(ours, theirs, Known::Nothing, newer);
        assert!(!merged.lost_default && merged.lost_profile, "the conflict");
        let (new_ours, new_theirs) = texts(merged);
        assert_eq!(
            new_ours.unwrap(),
            r#"{"id":"default","projects":{"C:/a":{"n":1},"C:/both":{"n":1},"C:/b":{"n":3}},"mcpServers":{"docs":1,"tickets":2}}"#
        );
        assert_eq!(
            new_theirs.unwrap(),
            r#"{"mcpServers":{"tickets":2,"docs":1},"id":"team","projects":{"C:/both":{"n":1},"C:/b":{"n":3},"C:/a":{"n":1}}}"#
        );
    }
    // With a base (here one that knows nothing) the newer file wins.
    let (new_ours, new_theirs) = texts(merge(ours, theirs, Known::Values(&[]), false));
    assert!(new_ours.unwrap().contains(r#""C:/both":{"n":2}"#));
    assert!(new_theirs.unwrap().contains(r#""C:/both":{"n":2}"#));
    // The same values in another layout are no change, on either side.
    let same = merge(
        "{\"mcpServers\": {\"a\": 1,\n \"b\": [1, 2]}}",
        r#"{"mcpServers":{"b":[1,2],"a":1},"id":"keep"}"#,
        Known::Values(&[]),
        false,
    );
    assert_eq!(texts(same), (None, None));
    assert_eq!(texts(merge("{}", "{}", Known::Nothing, true)), (None, None));
    // A key one side lacks arrives whole.
    let (new_ours, new_theirs) = texts(merge(
        r#"{"mcpServers":{"a":1}}"#,
        r#"{"projects":{"p":{"n":[1,2]}},"x":true}"#,
        Known::Nothing,
        true,
    ));
    assert_eq!(
        new_ours.unwrap(),
        r#"{"mcpServers":{"a":1},"projects":{"p":{"n":[1,2]}}}"#
    );
    assert_eq!(
        new_theirs.unwrap(),
        r#"{"projects":{"p":{"n":[1,2]}},"x":true,"mcpServers":{"a":1}}"#
    );
    // A shared key that holds no object is not merged, copied or replaced.
    for odd in [
        r#"{"projects":[1]}"#,
        r#"{"projects":null}"#,
        r#"{"mcpServers":{},"projects":"x"}"#,
    ] {
        let fine = r#"{"projects":{"p":{}}}"#;
        let refused = Err(Refused::NoObject("projects".into()));
        for (ours, theirs) in [(odd, fine), (fine, odd)] {
            let (ours, theirs) = (Some(ours.as_bytes()), Some(theirs.as_bytes()));
            assert_eq!(
                merge_keys(ours, theirs, &keys(), Known::Nothing, true),
                refused
            );
        }
    }
    // A profile without a state file gets one with the shared keys only.
    let source = br#"{"mcpServers":{"a":1},"oauthAccount":{"emailAddress":"max@example.test"}}"#;
    let created = merge_keys(Some(source), None, &keys(), Known::Nothing, true).unwrap();
    assert_eq!(created.default, None);
    assert_eq!(created.shared, nodes(r#"{"mcpServers":{"a":1}}"#));
    assert_eq!(
        text(created.profile).unwrap(),
        "{\n  \"mcpServers\": {\n    \"a\": 1\n  }\n}"
    );
    assert_eq!(
        merge_keys(None, None, &keys(), Known::Nothing, true),
        Ok(Default::default())
    );
    // A pretty file keeps its indentation for the entries it gains.
    let pretty = "{\n  \"mcpServers\": {\n    \"a\": 1\n  }\n}\n";
    let grown = merge(
        pretty,
        r#"{"mcpServers":{"b":{"x":1}}}"#,
        Known::Nothing,
        true,
    );
    assert_eq!(
        text(grown.default).unwrap(),
        "{\n  \"mcpServers\": {\n    \"a\": 1,\n    \"b\": {\n      \"x\": 1\n    }\n  }\n}\n"
    );
    // Anything that is no JSON object stops the merge.
    for broken in ["", "{ nope", "[1]", "null", "{\"a\":1} trailing"] {
        let broken = Some(broken.as_bytes());
        let fine: Option<&[u8]> = Some(b"{\"mcpServers\":{}}");
        for (ours, theirs) in [(broken, fine), (fine, broken)] {
            let merged = merge_keys(ours, theirs, &keys(), Known::Nothing, true);
            assert_eq!(merged, Err(Refused::NotJson));
        }
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
        // The field only the profile changed is the profile's, although the default
        // home's file is the newer one.
        assert_eq!(
            value["projects"]["C:/Code/demo"]["hasTrustDialogAccepted"], false,
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

fn names(servers: &[&str]) -> Base {
    let names = servers.iter().map(|name| name.to_string()).collect();
    Base::from([("mcpServers".to_string(), names)])
}

/// A removal is told from an addition by what is known of the last sync: the value base,
/// or — once, for a marker from before it — the entry names alone.
#[test]
fn a_base_tells_a_removed_entry_from_an_added_one() {
    let listed = names(&["docs", "old", "tickets"]);
    let values = nodes(r#"{"mcpServers":{"docs":1,"old":3,"tickets":2}}"#);
    // Removed in the default home (`old`), removed in the profile (`tickets`), added on
    // each side (`new-d`, `new-p`), kept on both (`docs`).
    let ours = r#"{"id":"default","mcpServers":{"docs":1,"tickets":2,"new-d":4}}"#;
    let theirs = r#"{"mcpServers":{"docs":1,"old":3,"new-p":5},"id":"team"}"#;
    for known in [Known::Values(&values), Known::Names(&listed)] {
        let merged = merge(ours, theirs, known, true);
        assert_eq!(
            text(merged.default.clone()).unwrap(),
            r#"{"id":"default","mcpServers":{"docs":1,"new-d":4,"new-p":5}}"#
        );
        assert_eq!(
            text(merged.profile.clone()).unwrap(),
            r#"{"mcpServers":{"docs":1,"new-p":5,"new-d":4},"id":"team"}"#
        );
        assert!(merged.lost_default && merged.lost_profile);
        assert_eq!(
            Node::Object(merged.shared),
            Node::Object(nodes(r#"{"mcpServers":{"docs":1,"new-d":4,"new-p":5}}"#))
        );
        // Removing the only entry leaves an empty object, not a missing key.
        let only = (nodes(r#"{"mcpServers":{"old":3}}"#), names(&["old"]));
        let known = match known {
            Known::Values(_) => Known::Values(&only.0),
            _ => Known::Names(&only.1),
        };
        let last = merge(
            r#"{"mcpServers":{}}"#,
            r#"{"mcpServers":{"old":3}}"#,
            known,
            true,
        );
        assert_eq!(text(last.profile).unwrap(), r#"{"mcpServers":{}}"#);
        assert_eq!(last.shared, nodes(r#"{"mcpServers":{}}"#));
    }
    // With names alone the removal wins even when the other side changed the entry since;
    // with the values that side's change is kept.
    let (kept, changed) = (
        r#"{"mcpServers":{"docs":1}}"#,
        r#"{"mcpServers":{"docs":1,"old":99}}"#,
    );
    let by_name = merge(kept, changed, Known::Names(&listed), true);
    assert_eq!(text(by_name.profile).unwrap(), kept);
    assert_eq!(by_name.default, None);
    let by_value = merge(kept, changed, Known::Values(&values), true);
    assert_eq!(text(by_value.default).unwrap(), changed);
    assert_eq!(by_value.profile, None);
    // The same input without a base, or with one from another key, is a plain union.
    let nothing = Base::new();
    for known in [Known::Nothing, Known::Names(&nothing), Known::Values(&[])] {
        let united = merge(ours, theirs, known, true);
        assert!(!united.lost_default && !united.lost_profile);
        let value: Value = serde_json::from_slice(&united.default.unwrap()).unwrap();
        assert_eq!(
            value["mcpServers"],
            json!({"docs":1,"tickets":2,"new-d":4,"old":3,"new-p":5})
        );
    }
    // A missing file or a missing key has removed nothing.
    let source = r#"{"mcpServers":{"docs":1,"old":3}}"#;
    for known in [Known::Values(&values), Known::Names(&listed)] {
        let created = merge_keys(Some(source.as_bytes()), None, &keys(), known, true).unwrap();
        assert!(text(created.profile).unwrap().contains("old"));
        assert_eq!(created.default, None);
        let keyless = merge(source, r#"{"id":"team"}"#, known, true);
        assert_eq!(keyless.default, None);
        assert!(text(keyless.profile).unwrap().contains("old") && !keyless.lost_default);
    }
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
    // The base is a file of its own; the marker holds neither names nor values.
    assert_eq!(read_marker(&world.dir("team")).unwrap().base, None);
    assert!(!read(&world.dir("team").join(MARKER)).contains("command"));
    let base_file = world.dir("team").join(".traygent-base/.claude.json");
    let known: Value = serde_json::from_str(&read(&base_file)).unwrap();
    assert_eq!(known["mcpServers"].as_object().unwrap().len(), 3);
    assert_eq!(known.get("id"), None, "the shared keys only");
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
    // An upgrade: the marker of the previous version holds the entry names and there is
    // no value base yet. The names still tell the removal — once.
    std::fs::remove_file(&base_file).unwrap();
    write(
        &world.dir("team").join(MARKER),
        r#"{"version":1,"syncedAt":"2026-11-13T09:00:00Z","done":[".claude.json"],"base":{"mcpServers":["docs","new-d","new-p"],"projects":[]}}"#,
    );
    write(&world.state("team"), &state(&["docs", "new-p"], "team"));
    world.synced(&["team"], 4);
    assert_eq!(servers("default"), ["docs", "new-p"]);
    assert_eq!(read_marker(&world.dir("team")).unwrap().base, None);
    assert!(base_file.exists(), "the value base takes over");
    // Neither a base nor names: the next sync unites and never removes.
    std::fs::remove_file(&base_file).unwrap();
    write(&world.state("team"), &state(&["docs"], "team"));
    world.synced(&["team"], 5);
    assert_eq!(servers("team"), ["docs", "new-p"]);
    assert_eq!(servers("default"), ["docs", "new-p"]);
}

/// A state file that was recreated holds none of the names the base knows: that is a
/// reset, not a row of removals, and nothing is removed anywhere.
#[test]
fn a_reset_side_removes_nothing_but_a_partial_removal_still_travels() {
    let full = r#"{"mcpServers":{"a":1,"b":2,"c":3}}"#;
    let (values, listed) = (nodes(full), names(&["a", "b", "c"]));
    for known in [Known::Values(&values), Known::Names(&listed)] {
        // Either side wiped, with or without something new in it: united.
        for wiped in [r#"{"mcpServers":{}}"#, r#"{"mcpServers":{"new":9}}"#] {
            for (ours, theirs) in [(full, wiped), (wiped, full)] {
                let merged = merge(ours, theirs, known, true);
                assert!(!merged.lost_default && !merged.lost_profile, "{wiped}");
                for side in [merged.default, merged.profile].into_iter().flatten() {
                    let value: Value = serde_json::from_slice(&side).unwrap();
                    for name in ["a", "b", "c"] {
                        assert!(value["mcpServers"].get(name).is_some(), "{name} in {value}");
                    }
                }
                let recorded = merged.shared[0].1.as_object().unwrap().len();
                assert!(recorded >= 3, "the base is recorded as usual");
            }
        }
        // One of three removed: still a removal, in either direction.
        let two = r#"{"mcpServers":{"a":1,"c":3}}"#;
        let merged = merge(two, full, known, true);
        assert_eq!(text(merged.profile).unwrap(), two);
        assert!(merged.lost_profile && merged.default.is_none());
        let merged = merge(full, two, known, true);
        assert_eq!(text(merged.default).unwrap(), two);
        assert_eq!(merged.shared, nodes(two));
        // Two of three removed too — one known name is left on that side.
        let one = r#"{"mcpServers":{"b":2}}"#;
        assert_eq!(text(merge(full, one, known, true).default).unwrap(), one);
    }

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
