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
