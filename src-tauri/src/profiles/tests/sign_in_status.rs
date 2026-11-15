//! The sign-in check of a profile's credentials file (#43): only `claudeAiOauth.refreshToken`
//! (present, not blank) and `subscriptionType` are looked at. Synthetic values only.
use super::*;

const TOKEN: &str = "synthetic-refresh-not-real";

fn facts_with(name: &str, credentials: Option<&[u8]>) -> Facts {
    let world = World::new(name);
    std::fs::create_dir_all(world.dir("default")).unwrap();
    if let Some(bytes) = credentials {
        std::fs::write(world.dir("default").join(".credentials.json"), bytes).unwrap();
    }
    status::read(&world.homes, CLAUDE, "default")
}

fn signed(name: &str, json: &str) -> (bool, Option<String>) {
    let facts = facts_with(name, Some(json.as_bytes()));
    (facts.signed_in, facts.plan)
}

#[test]
fn a_missing_file_or_one_without_a_refresh_token_is_signed_out() {
    assert_eq!(
        facts_with("status-missing", None),
        Facts::default(),
        "no file"
    );
    assert_eq!(signed("status-empty-object", "{}"), (false, None));
    assert_eq!(
        signed("status-null-oauth", r#"{"claudeAiOauth":null}"#),
        (false, None)
    );
    for token in [r#""""#, r#""   ""#, "null"] {
        let json = format!(
            r#"{{"claudeAiOauth":{{"accessToken":"x","refreshToken":{token},"subscriptionType":"max"}}}}"#
        );
        assert_eq!(
            signed("status-blank-token", &json),
            (false, None),
            "{token}"
        );
    }
    let without = r#"{"claudeAiOauth":{"subscriptionType":"pro"}}"#;
    assert_eq!(signed("status-no-token", without), (false, None));
}

#[test]
fn a_refresh_token_signs_in_and_only_the_plan_leaves() {
    let json = format!(
        "\u{feff}{{\"claudeAiOauth\":{{\"accessToken\":\"{TOKEN}-a\",\"refreshToken\":\"{TOKEN}\",\
         \"expiresAt\":1,\"scopes\":[\"user:inference\"],\"subscriptionType\":\"max\"}}}}"
    );
    let facts = facts_with("status-signed-in", Some(json.as_bytes()));
    assert!(facts.signed_in);
    assert_eq!(facts.plan.as_deref(), Some("max"));
    assert!(!format!("{facts:?}").contains(TOKEN));
    // Without a plan, still signed in; the plan text is cleaned like the account.
    let bare = format!(r#"{{"claudeAiOauth":{{"refreshToken":"{TOKEN}"}}}}"#);
    assert_eq!(signed("status-no-plan", &bare), (true, None));
    let noisy = format!(
        r#"{{"claudeAiOauth":{{"refreshToken":"{TOKEN}","subscriptionType":" team\u0007 "}}}}"#
    );
    assert_eq!(
        signed("status-noisy-plan", &noisy),
        (true, Some("team".into()))
    );
}

#[test]
fn a_file_that_does_not_parse_or_is_too_large_stays_signed_in_while_not_empty() {
    // The CLI may be mid-write: a race never flips the row.
    assert_eq!(
        signed("status-garbage", "{\"claudeAiOauth\":{\"refre"),
        (true, None)
    );
    assert_eq!(signed("status-text", "synthetic"), (true, None));
    assert!(!facts_with("status-zero", Some(b"")).signed_in);
    let mut huge = br#"{"claudeAiOauth":{"refreshToken":"","pad":""#.to_vec();
    huge.resize(1024 * 1024 + 1, b'x');
    huge.extend_from_slice(b"\"}}");
    assert_eq!(
        facts_with("status-oversize", Some(&huge)),
        Facts {
            signed_in: true,
            ..Facts::default()
        },
        "not parsed"
    );
}
