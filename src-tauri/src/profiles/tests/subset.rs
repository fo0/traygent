//! The state file's shared subset merged three ways against a value base (#35): edits on
//! both sides survive, removals travel, and nothing of a profile's identity or runtime
//! state reaches the base. Every account, id and session here is made up.
use super::*;
use crate::profiles::pair::AFTER_READ;
use serde_json::{json, Value};

mod seed;

const DEMO: &str = "C:/Code/demo";

fn parsed(path: &Path) -> Value {
    serde_json::from_str(read(path).trim_start_matches('\u{feff}')).unwrap()
}

fn base(world: &World, id: &str) -> PathBuf {
    world.dir(id).join(".traygent-base/.claude.json")
}

/// Writes a profile's state file, pretty, with a fixed file time.
fn put(world: &World, id: &str, value: &Value, seconds: u64) {
    write(
        &world.state(id),
        &serde_json::to_string_pretty(value).unwrap(),
    );
    touch(&world.state(id), seconds);
}

fn stamp(path: &Path) -> (String, SystemTime) {
    (
        read(path),
        std::fs::metadata(path).unwrap().modified().unwrap(),
    )
}

/// A state file with an identity and one project.
fn state(account: &str, project: Value) -> Value {
    json!({
        "userID": format!("id-of-{account}"),
        "oauthAccount": {"emailAddress": account},
        "showSpinnerTree": true,
        "mcpServers": {"docs": {"command": "docs-mcp", "lastUsed": 5}},
        "projects": {DEMO: project},
    })
}

#[test]
fn both_sides_edits_of_one_project_survive_and_the_base_holds_the_subset_only() {
    let world = World::new("subset-both");
    let project = json!({"allowedTools": ["Read"], "hasTrustDialogAccepted": false, "lastCost": 1});
    put(
        &world,
        "default",
        &state("max@example.test", project.clone()),
        10,
    );
    put(&world, "team", &state("team@example.test", project), 20);
    world.synced(&["team"], 1);
    // The base: the shared keys, project entries without their runtime fields — and
    // nothing of the identity. `mcpServers` entries are not stripped.
    let known = read(&base(&world, "team"));
    for private in [
        "oauthAccount",
        "userID",
        "example.test",
        "lastCost",
        "hasUnseen",
    ] {
        assert!(!known.contains(private), "{private} in {known}");
    }
    assert_eq!(
        serde_json::from_str::<Value>(&known).unwrap(),
        json!({
            "mcpServers": {"docs": {"command": "docs-mcp", "lastUsed": 5}},
            "projects": {DEMO: {"allowedTools": ["Read"], "hasTrustDialogAccepted": false}},
            "showSpinnerTree": true
        })
    );
    assert!(!base(&world, "default").exists());
    // Each side changes another real field; the default home's file is the newer one only
    // because a session rewrote its runtime fields.
    let mut ours = parsed(&world.state("default"));
    ours["projects"][DEMO]["hasTrustDialogAccepted"] = json!(true);
    ours["projects"][DEMO]["lastCost"] = json!(7.5);
    ours["projects"][DEMO]["lastSessionId"] = json!("session-default");
    let mut theirs = parsed(&world.state("team"));
    theirs["projects"][DEMO]["allowedTools"] = json!(["Read", "Edit"]);
    theirs["projects"][DEMO]["lastCost"] = json!(2);
    put(&world, "team", &theirs, 100);
    put(&world, "default", &ours, 900);
    world.synced(&["team"], 2);
    let real = json!({"allowedTools": ["Read", "Edit"], "hasTrustDialogAccepted": true});
    let with = |fields: Value| {
        let mut entry = real.clone();
        entry
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        entry
    };
    assert_eq!(
        parsed(&world.state("default"))["projects"][DEMO],
        with(json!({"lastCost": 7.5, "lastSessionId": "session-default"}))
    );
    assert_eq!(
        parsed(&world.state("team"))["projects"][DEMO],
        with(json!({"lastCost": 2}))
    );
    assert_eq!(parsed(&base(&world, "team"))["projects"][DEMO], real);
    // Nobody lost anything: no backup. And the next run has nothing to do.
    for id in ["default", "team"] {
        assert!(!world.dir(id).join(".traygent-backup").exists(), "{id}");
        assert_eq!(
            parsed(&world.state(id))["oauthAccount"]["emailAddress"],
            format!("{}@example.test", if id == "team" { "team" } else { "max" })
        );
    }
    assert!(!world.synced(&["team"], 3)[0].changed);
    // Both changed the same field: the newer file's value, the other file in its backup.
    ours = parsed(&world.state("default"));
    theirs = parsed(&world.state("team"));
    ours["projects"][DEMO]["allowedTools"] = json!(["Bash"]);
    theirs["projects"][DEMO]["allowedTools"] = json!(["Write"]);
    put(&world, "default", &ours, 1000);
    put(&world, "team", &theirs, 1100);
    world.synced(&["team"], 4);
    for id in ["default", "team"] {
        assert_eq!(
            parsed(&world.state(id))["projects"][DEMO]["allowedTools"],
            json!(["Write"])
        );
    }
    let kept = parsed(&world.backup("default").join(".claude.json"));
    assert_eq!(kept["projects"][DEMO]["allowedTools"], json!(["Bash"]));
}

#[test]
fn removals_travel_through_the_value_base() {
    let world = World::new("subset-removals");
    let mut both = state(
        "max@example.test",
        json!({"allowedTools": ["Read"], "lastCost": 1}),
    );
    both["projects"]["C:/Code/other"] = json!({"allowedTools": []});
    both["claudeInChromeDefaultEnabled"] = json!(false);
    put(&world, "default", &both, 10);
    world.synced(&["team", "solo"], 1);
    // The default home drops a whole-value key and a project; the profile still has that
    // project with runtime fields a session wrote since.
    let mut ours = parsed(&world.state("default"));
    ours.as_object_mut().unwrap().remove("showSpinnerTree");
    ours["projects"].as_object_mut().unwrap().remove(DEMO);
    put(&world, "default", &ours, 100);
    let mut theirs = parsed(&world.state("team"));
    theirs["projects"][DEMO]["lastCost"] = json!(42);
    theirs["projects"][DEMO]["lastSessionId"] = json!("session-team");
    put(&world, "team", &theirs, 900);
    world.synced(&["team", "solo"], 2);
    for id in ["default", "team", "solo"] {
        let value = parsed(&world.state(id));
        assert_eq!(value.get("showSpinnerTree"), None, "{id}");
        assert_eq!(value["claudeInChromeDefaultEnabled"], false, "{id}");
        let projects: Vec<&String> = value["projects"].as_object().unwrap().keys().collect();
        assert_eq!(projects, ["C:/Code/other"], "{id}");
    }
    // What the profile lost — runtime fields included — is in its own backup.
    let kept = parsed(&world.backup("team").join(".claude.json"));
    assert_eq!(kept["projects"][DEMO]["lastSessionId"], "session-team");
    assert!(!world
        .synced(&["team", "solo"], 3)
        .iter()
        .any(|report| report.changed));
    // A profile added now, whose file still has the removed project, does not bring it
    // back: the other profiles' base knows the default home removed it … once it is gone
    // from every base it would be an addition again.
    let world = World::new("subset-seed");
    put(&world, "default", &both, 10);
    world.synced(&["team"], 1);
    put(&world, "default", &ours, 100);
    put(&world, "solo", &both, 900);
    world.synced(&["team", "solo"], 2);
    for id in ["default", "team", "solo"] {
        let value = parsed(&world.state(id));
        assert_eq!(value["projects"].get(DEMO), None, "{id}");
        assert_eq!(value.get("showSpinnerTree"), None, "{id}");
    }
}

#[test]
fn a_rebuilt_entry_is_valid_in_any_layout_and_does_not_go_back_and_forth() {
    let entry = r#"{"allowedTools":["Read"],"lastCost":0.1234567890123456789,"n":123456789012345678901234567890}"#;
    let single = format!(r#"{{"userID":"a","projects":{{"{DEMO}":{entry}}}}}"#);
    let crlf = format!("{{\r\n  \"userID\": \"a\",\r\n  \"projects\": {{\r\n    \"{DEMO}\": {entry}\r\n  }}\r\n}}\r\n");
    let tabs = format!(
        "{{\n\t\"userID\": \"a\",\n\t\"projects\": {{\n\t\t\"{DEMO}\": {entry}\n\t}}\n}}\n"
    );
    for (name, layout) in [("single", single), ("crlf", crlf), ("tabs", tabs)] {
        let world = World::new(&format!("subset-layout-{name}"));
        write(&world.state("default"), &layout);
        world.synced(&["team"], 1);
        assert_eq!(read(&world.state("default")), layout, "{name}");
        // The profile changes a real field and adds a project.
        let added = json!({"allowedTools": [], "nested": {"a": [1, {"b": null}]}});
        let theirs = format!(
            r#"{{"projects":{{"{DEMO}":{{"allowedTools":["Read","Edit"],"n":123456789012345678901234567890}},"C:/Code/new":{added}}}}}"#
        );
        write(&world.state("team"), &theirs);
        touch(&world.state("team"), 900);
        touch(&world.state("default"), 10);
        world.synced(&["team"], 2);
        let text = read(&world.state("default"));
        let value: Value =
            serde_json::from_str(&text).unwrap_or_else(|error| panic!("{name}: {error}\n{text}"));
        assert_eq!(
            value["projects"][DEMO]["allowedTools"],
            json!(["Read", "Edit"]),
            "{name}"
        );
        assert_eq!(value["projects"]["C:/Code/new"], added, "{name}");
        // The file's own layout, and the number tokens as they were written.
        match name {
            "single" => assert!(!text.contains('\n') && !text.contains(": "), "{text}"),
            "crlf" => assert!(
                text.ends_with("}\r\n") && !text.replace("\r\n", "").contains('\n'),
                "{text}"
            ),
            _ => assert!(
                text.contains("\n\t\t\t\"allowedTools\": [") && !text.contains("  "),
                "{text}"
            ),
        }
        assert!(text.contains("0.1234567890123456789"), "{name}: {text}");
        assert!(
            text.contains("123456789012345678901234567890"),
            "{name}: {text}"
        );
        // No ping-pong: the next runs write nothing on either side.
        let before = (stamp(&world.state("default")), stamp(&world.state("team")));
        for minute in [3, 4] {
            assert!(!world.synced(&["team"], minute)[0].changed, "{name}");
        }
        assert_eq!(
            (stamp(&world.state("default")), stamp(&world.state("team"))),
            before
        );
    }
}

#[test]
fn odd_values_are_left_alone_or_merged_as_plain_values() {
    // A shared key that is no object is left as it is on both sides and reported; the
    // other shared keys are merged and the rule has had its sync.
    let world = World::new("subset-null");
    let ours = r#"{"projects":null,"mcpServers":{"docs":{}},"showSpinnerTree":true}"#;
    let theirs = r#"{"projects":{"C:/Code/demo":{}},"mcpServers":{}}"#;
    write(&world.state("default"), ours);
    write(&world.state("team"), theirs);
    for minute in [1, 2] {
        let reports = world.sync(&["team"], minute);
        assert_eq!(
            reports[0].errors,
            [".claude.json: „projects“ ist kein Objekt und bleibt unverändert"]
        );
        assert_eq!(read(&world.state("default")), ours);
        assert_eq!(
            parsed(&world.state("team")),
            json!({"projects": {DEMO: {}}, "mcpServers": {"docs": {}}, "showSpinnerTree": true})
        );
        assert_eq!(
            parsed(&base(&world, "team")),
            json!({"mcpServers": {"docs": {}}, "showSpinnerTree": true})
        );
    }
    let marker = read_marker(&world.dir("team")).unwrap();
    assert!(marker.done.iter().any(|rule| rule == ".claude.json"));
    assert_eq!(marker.synced_at, None, "not a sync without an error");
    // Once the key is an object again it is merged — united, the base knows nothing of it.
    write(
        &world.state("default"),
        r#"{"projects":{"C:/Code/a":{}},"mcpServers":{"docs":{}},"showSpinnerTree":true}"#,
    );
    world.synced(&["team"], 3);
    for id in ["default", "team"] {
        let projects = parsed(&world.state(id))["projects"]
            .as_object()
            .unwrap()
            .len();
        assert_eq!(projects, 2, "{id}");
    }

    // An entry that is no object on one side is one value: it follows the side that
    // changed it, and no runtime field of the other side's entry is copied anywhere.
    let world = World::new("subset-entry");
    put(
        &world,
        "default",
        &json!({"projects": {DEMO: {"allowedTools": ["Read"], "lastCost": 3}}}),
        10,
    );
    world.synced(&["team"], 1);
    put(&world, "team", &json!({"projects": {DEMO: "legacy"}}), 900);
    world.synced(&["team"], 2);
    assert_eq!(parsed(&world.state("default"))["projects"][DEMO], "legacy");
    put(
        &world,
        "default",
        &json!({"projects": {DEMO: {"allowedTools": [], "lastCost": 9}}}),
        950,
    );
    world.synced(&["team", "solo"], 3);
    for id in ["team", "solo"] {
        assert_eq!(
            parsed(&world.state(id))["projects"][DEMO],
            json!({"allowedTools": []}),
            "{id}"
        );
    }
    // `mcpServers` entries keep fields that only look like runtime fields.
    assert_eq!(
        recipe(CLAUDE).unwrap().own_fields.as_ref().unwrap().key,
        "projects"
    );
    let world = World::new("subset-servers");
    put(
        &world,
        "default",
        &json!({"mcpServers": {"docs": {"lastUsed": 1, "hasUnseenTeamArtifacts": true}}}),
        10,
    );
    world.synced(&["team"], 1);
    assert_eq!(
        parsed(&world.state("team"))["mcpServers"]["docs"],
        json!({"lastUsed": 1, "hasUnseenTeamArtifacts": true})
    );
}

#[test]
fn a_state_file_written_meanwhile_or_too_large_a_subset_keeps_things_safe() {
    let world = World::new("subset-meanwhile");
    put(&world, "default", &json!({"mcpServers": {"docs": {}}}), 10);
    world.synced(&["team"], 1);
    let before = read(&base(&world, "team"));
    put(
        &world,
        "team",
        &json!({"mcpServers": {"docs": {}, "new": {}}}),
        200,
    );
    let file = world.state("default");
    AFTER_READ.with(|hook| {
        let (file, calls) = (file.clone(), std::cell::Cell::new(0));
        *hook.borrow_mut() = Some(Box::new(move || {
            // The second merged rule of the pair is the state file's.
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                write(
                    &file,
                    r#"{"mcpServers":{"docs":{}},"note":"a session wrote this"}"#,
                );
            }
        }));
    });
    let reports = world.sync(&["team"], 2);
    AFTER_READ.with(|hook| *hook.borrow_mut() = None);
    assert_eq!(
        reports[0].errors,
        [".claude.json: wurde gerade geändert, nächster Versuch beim nächsten Abgleich"]
    );
    assert!(read(&file).contains("a session wrote this"));
    assert_eq!(read(&base(&world, "team")), before);
    world.synced(&["team"], 3);
    assert_eq!(parsed(&file)["mcpServers"], json!({"docs": {}, "new": {}}));
    assert_eq!(parsed(&file)["note"], "a session wrote this");

    // A subset beyond the size cap gets an empty base: the next sync unites and removes
    // nothing, as without one.
    let large = World::new("subset-large");
    let big = "x".repeat(8 * 1024 * 1024);
    put(
        &large,
        "default",
        &json!({"mcpServers": {"a": {"blob": big}, "b": {}, "c": {}}}),
        10,
    );
    large.synced(&["team"], 1);
    assert_eq!(read(&base(&large, "team")), "{}\n");
    put(&large, "team", &json!({"mcpServers": {"c": {}}}), 900);
    large.synced(&["team"], 2);
    let servers = parsed(&large.state("default"))["mcpServers"]
        .as_object()
        .unwrap()
        .len();
    assert_eq!(servers, 3);
    assert_eq!(
        parsed(&large.state("team"))["mcpServers"]
            .as_object()
            .unwrap()
            .len(),
        3
    );
}
