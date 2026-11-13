//! The state file's shared keys: united entry by entry, everything else left alone.
use super::*;
use crate::profiles::json::{merge_keys, NotJson};
use serde_json::{json, Value};

fn parsed(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap()
}

fn text(bytes: Option<Vec<u8>>) -> Option<String> {
    bytes.map(|bytes| String::from_utf8(bytes).unwrap())
}

#[test]
fn shared_keys_are_united_entry_by_entry() {
    let keys = ["mcpServers", "projects"];
    let merge = |ours: &str, theirs: &str, default_wins: bool| {
        let (ours, theirs) = merge_keys(
            Some(ours.as_bytes()),
            Some(theirs.as_bytes()),
            &keys,
            default_wins,
        )
        .unwrap();
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
    let (unchanged, created) = merge_keys(Some(source), None, &keys, true).unwrap();
    assert_eq!(unchanged, None);
    assert_eq!(text(created).unwrap(), "{\n  \"mcpServers\": {\"a\":1}\n}");
    assert_eq!(merge_keys(None, None, &keys, true), Ok((None, None)));
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
        assert_eq!(merge_keys(broken, fine, &keys, true), Err(NotJson));
        assert_eq!(merge_keys(fine, broken, &keys, true), Err(NotJson));
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
