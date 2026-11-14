//! When a watch commits, waits or ends. Synthetic data only.
use super::*;

fn key(id: &str) -> Key {
    (AgentId::Claude, id.into())
}

fn signed(account: Option<&str>, organization: Option<&str>) -> Facts {
    Facts {
        signed_in: true,
        account: account.map(Into::into),
        organization: organization.map(Into::into),
        synced_at: None,
    }
}

#[test]
fn only_a_changed_sign_in_commits_and_the_limit_ends_the_watch() {
    let now = Instant::now();
    assert_eq!(WATCH_LIMIT, Duration::from_secs(600));
    let watch = start(key("step"), Facts::default(), now);
    assert_eq!(watch.step(Some(&Facts::default()), now), Step::Wait);
    // A newer sync time is no sign-in.
    let synced = Facts {
        synced_at: Some("2026-11-14T09:00:00Z".into()),
        ..Facts::default()
    };
    assert_eq!(watch.step(Some(&synced), now + WATCH_TICK), Step::Wait);
    assert_eq!(watch.step(Some(&signed(None, None)), now), Step::Changed);
    assert_eq!(
        watch.step(Some(&Facts::default()), now + WATCH_LIMIT - WATCH_TICK),
        Step::Wait
    );
    assert_eq!(
        watch.step(Some(&Facts::default()), now + WATCH_LIMIT),
        Step::Stop
    );
    // A change still counts on the last read.
    assert_eq!(
        watch.step(Some(&signed(None, None)), now + WATCH_LIMIT),
        Step::Changed
    );
    // The profile was removed meanwhile.
    assert_eq!(watch.step(None, now), Step::Stop);
    // Signed in before: another account or organization is a new sign-in too.
    let before = signed(Some("a@example.com"), Some("Beispiel GmbH"));
    let again = start(key("step-again"), before.clone(), now);
    assert_eq!(again.step(Some(&before), now), Step::Wait);
    assert_eq!(
        again.step(Some(&signed(Some("a@example.com"), Some("Team"))), now),
        Step::Changed
    );
    assert_eq!(
        again.step(
            Some(&signed(Some("b@example.com"), Some("Beispiel GmbH"))),
            now
        ),
        Step::Changed
    );
}

#[test]
fn a_second_watch_of_a_profile_replaces_the_first() {
    let now = Instant::now();
    let first = start(key("replace"), Facts::default(), now);
    assert!(first.current() && watching(&key("replace")));
    let second = start(key("replace"), Facts::default(), now);
    assert!(!first.current());
    assert!(second.current());
    // The replaced one ending leaves the newer one registered.
    drop(first);
    assert!(second.current());
    let other = start(key("replace-other"), Facts::default(), now);
    assert!(second.current() && other.current());
    drop(second);
    assert!(!watching(&key("replace")));
}
