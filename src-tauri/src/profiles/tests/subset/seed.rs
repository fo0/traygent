//! A state file pair without a base of its own: another profile's base, or none at all.
use super::*;

/// The seed of another profile removes from a new profile's state file only the entries
/// it knows, and nothing when the default home holds none of them.
#[test]
fn a_seed_removes_only_the_entries_it_knows() {
    let entry = |tool: &str| json!({"allowedTools": [tool]});
    let world = World::new("subset-seed-partial");
    put(
        &world,
        "default",
        &json!({"projects": {"p0": entry("Read")}, "mcpServers": {"docs": {}}}),
        10,
    );
    world.synced(&["team"], 1);
    // The default home has no `projects` any more; the new profile holds two of its own
    // and the old one with a changed field.
    put(&world, "default", &json!({"mcpServers": {"docs": {}}}), 100);
    let theirs = json!({
        "projects": {"p0": entry("Edit"), "p1": entry("Read"), "p2": entry("Bash")},
        "mcpServers": {"docs": {}}
    });
    put(&world, "solo", &theirs, 900);
    world.synced(&["team", "solo"], 2);
    for id in ["default", "team", "solo"] {
        assert_eq!(
            parsed(&world.state(id))["projects"],
            theirs["projects"],
            "{id}"
        );
    }
    // Held exactly as the seed knew it: the entry goes, the others stay.
    let exact = World::new("subset-seed-exact");
    put(
        &exact,
        "default",
        &json!({"projects": {"p0": entry("Read"), "keep": {}}}),
        10,
    );
    exact.synced(&["team"], 1);
    put(&exact, "default", &json!({"projects": {"keep": {}}}), 100);
    put(
        &exact,
        "solo",
        &json!({"projects": {"p0": entry("Read"), "keep": {}, "p1": {}}}),
        900,
    );
    exact.synced(&["team", "solo"], 2);
    for id in ["default", "team", "solo"] {
        assert_eq!(
            parsed(&exact.state(id))["projects"],
            json!({"keep": {}, "p1": {}}),
            "{id}"
        );
    }
    // The default home's state file recreated — none of the seed's entries left: the seed
    // tells nothing and the new profile's entries come back to every home.
    let reset = World::new("subset-seed-reset");
    let both = json!({"projects": {"p0": entry("Read"), "p1": entry("Edit")}});
    put(&reset, "default", &both, 10);
    reset.synced(&["team"], 1);
    put(&reset, "default", &json!({"projects": {}}), 100);
    put(&reset, "solo", &both, 900);
    reset.synced(&["solo", "team"], 2);
    for id in ["default", "team", "solo"] {
        assert_eq!(
            parsed(&reset.state(id))["projects"],
            both["projects"],
            "{id}"
        );
    }
    // An upgrade without base, names or seed: nothing is removed and the newer file wins.
    let upgrade = World::new("subset-upgrade");
    put(
        &upgrade,
        "default",
        &json!({"projects": {"p0": entry("Read"), "a": {}}}),
        10,
    );
    put(
        &upgrade,
        "team",
        &json!({"projects": {"p0": entry("Edit"), "b": {}}}),
        900,
    );
    write(
        &upgrade.dir("team").join(MARKER),
        r#"{"version":1,"done":[".claude.json"]}"#,
    );
    upgrade.synced(&["team"], 1);
    for id in ["default", "team"] {
        assert_eq!(
            parsed(&upgrade.state(id))["projects"],
            json!({"p0": entry("Edit"), "a": {}, "b": {}}),
            "{id}"
        );
    }
}

/// Against another profile's base an object that is one unit — the status line, an MCP
/// server, a project entry — is never reduced to the fields the base does not know: it
/// goes when the new profile holds it exactly as the base knew it, and stays whole when
/// the profile has its own variant.
#[test]
fn a_seed_never_reduces_a_unit_object_to_a_fragment() {
    let world = World::new("subset-seed-units");
    let settings = |id: &str| world.dir(id).join("settings.json");
    let line = json!({"type": "command", "command": "x", "padding": 0});
    let server = json!({"command": "srv", "args": ["a"]});
    let project = json!({"allowedTools": ["Read"], "hasTrustDialogAccepted": true});
    write(
        &settings("default"),
        &json!({"statusLine": line, "same": {"a": 1, "b": 2}, "env": {"KEEP": "1", "GONE": "2"}})
            .to_string(),
    );
    put(
        &world,
        "default",
        &json!({
            "mcpServers": {"m1": server, "m0": server, "keep": {}},
            "projects": {"p1": project, "p0": project, "keep": {}}
        }),
        10,
    );
    world.synced(&["team"], 1);
    // The default home removes the status line, `same`, a variable, two servers and two
    // projects.
    write(&settings("default"), r#"{"env":{"KEEP":"1"}}"#);
    put(
        &world,
        "default",
        &json!({"mcpServers": {"keep": {}}, "projects": {"keep": {}}}),
        100,
    );
    // The new profile holds its own variant of `statusLine`, `m1` and `p1` (a field
    // changed, a field dropped), and the others exactly as they were.
    let own_line = json!({"type": "command", "command": "y", "padding": 0});
    let own_server = json!({"command": "srv", "args": ["a", "b"]});
    let own_project =
        json!({"allowedTools": ["Bash"], "hasTrustDialogAccepted": true, "lastCost": 4});
    write(
        &settings("solo"),
        &json!({
            "statusLine": own_line, "same": {"a": 1, "b": 2},
            "env": {"KEEP": "1", "GONE": "2", "MINE": "3"}
        })
        .to_string(),
    );
    put(
        &world,
        "solo",
        &json!({
            "mcpServers": {"m1": own_server, "m0": server, "keep": {}},
            "projects": {"p1": own_project, "p0": project, "keep": {}}
        }),
        900,
    );
    world.synced(&["team", "solo"], 2);
    for id in ["default", "team", "solo"] {
        // Whole, in every home; what was unchanged is gone; `env` is a container.
        assert_eq!(
            parsed(&settings(id)),
            json!({"env": {"KEEP": "1", "MINE": "3"}, "statusLine": own_line}),
            "{id}"
        );
        let state = parsed(&world.state(id));
        assert_eq!(
            state["mcpServers"],
            json!({"keep": {}, "m1": own_server}),
            "{id}"
        );
        let entry = &state["projects"]["p1"];
        assert_eq!(entry["allowedTools"], json!(["Bash"]), "{id}");
        assert_eq!(entry["hasTrustDialogAccepted"], true, "{id}");
        let projects: Vec<&String> = state["projects"].as_object().unwrap().keys().collect();
        assert_eq!(projects, ["keep", "p1"], "{id}");
    }
    assert_eq!(
        parsed(&world.state("solo"))["projects"]["p1"]["lastCost"],
        4
    );
    assert_eq!(
        parsed(&world.state("default"))["projects"]["p1"].get("lastCost"),
        None
    );
    // A unit both homes hold in different variants is united field by field, the default
    // home winning — the seed says nothing about its inside.
    let both = World::new("subset-seed-both");
    let settings = |id: &str| both.dir(id).join("settings.json");
    write(
        &settings("default"),
        &json!({"statusLine": line}).to_string(),
    );
    both.synced(&["team"], 1);
    write(
        &settings("default"),
        r#"{"statusLine":{"type":"command","command":"z"}}"#,
    );
    write(
        &settings("solo"),
        &json!({"statusLine": own_line}).to_string(),
    );
    own(&both, "solo");
    both.synced(&["team", "solo"], 2);
    for id in ["default", "team", "solo"] {
        assert_eq!(
            parsed(&settings(id))["statusLine"],
            json!({"type": "command", "command": "z", "padding": 0}),
            "{id}"
        );
    }
}

#[test]
fn every_shared_key_that_is_no_object_is_reported() {
    let world = World::new("subset-odd-both");
    write(
        &world.state("default"),
        r#"{"projects":null,"showSpinnerTree":false}"#,
    );
    write(&world.state("team"), r#"{"mcpServers":[1],"projects":{}}"#);
    let reports = world.sync(&["team"], 1);
    assert_eq!(
        reports[0].errors,
        [
            ".claude.json: „projects“ ist kein Objekt und bleibt unverändert",
            ".claude.json: „mcpServers“ ist kein Objekt und bleibt unverändert"
        ]
    );
    assert_eq!(
        read(&world.state("default")),
        r#"{"projects":null,"showSpinnerTree":false}"#
    );
    assert_eq!(
        parsed(&world.state("team")),
        json!({"mcpServers": [1], "projects": {}, "showSpinnerTree": false})
    );
}
