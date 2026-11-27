use super::*;
use chrono::FixedOffset;
use serde_json::json;

const SUBSCRIPTION: &str = include_str!("../../../../fixtures/claude-usage/subscription.txt");
const API_KEY: &str = include_str!("../../../../fixtures/claude-usage/api-key.txt");
const PRINTED_API_KEY: &str = include_str!("../../../../fixtures/claude-usage/print-api-key.json");

/// Berlin in winter: UTC+1.
fn berlin(local: &str) -> DateTime<FixedOffset> {
    let zone = FixedOffset::east_opt(3600).unwrap();
    let at = NaiveDateTime::parse_from_str(local, "%Y-%m-%d %H:%M").unwrap();
    zone.from_local_datetime(&at).unwrap()
}

fn windows(text: &str, now: &DateTime<FixedOffset>) -> serde_json::Value {
    serde_json::to_value(parse(text, now)).unwrap()
}

#[test]
fn the_subscription_sample_gives_the_week_and_fable_windows() {
    let quota = windows(SUBSCRIPTION, &berlin("2026-11-27 18:00"));
    assert_eq!(
        quota,
        json!({"windows": {
            "week": {"used": 82.0, "resetAt": "2026-11-29T06:59:00Z"},
            "fable": {"used": 74.0, "resetAt": "2026-11-29T07:00:00Z"}
        }, "detail": null})
    );
}

#[test]
fn the_api_key_sample_has_no_subscription() {
    let now = berlin("2026-11-27 18:00");
    assert_eq!(
        parse(API_KEY, &now),
        AgentQuota::unavailable(NO_SUBSCRIPTION)
    );
    // The JSON object the CLI printed in a cloud container without a subscription.
    let text = result_text(PRINTED_API_KEY.as_bytes()).unwrap();
    assert!(text.starts_with("Estimated cost:"));
    assert_eq!(parse(&text, &now), AgentQuota::unavailable(NO_SUBSCRIPTION));
}

#[test]
fn reset_times_with_and_without_minutes_and_a_time_alone() {
    let now = berlin("2026-11-27 13:10");
    for (when, expected) in [
        (
            "Nov 29, 7:59am (Europe/Berlin)",
            Some("2026-11-29T06:59:00Z"),
        ),
        ("Nov 29, 8am (Europe/Berlin)", Some("2026-11-29T07:00:00Z")),
        ("Nov 29, 12am (Europe/Berlin)", Some("2026-11-28T23:00:00Z")),
        ("Nov 29, 12pm (Europe/Berlin)", Some("2026-11-29T11:00:00Z")),
        ("Nov 29, 15:30", Some("2026-11-29T14:30:00Z")),
        // A time alone: today's, tomorrow's once more than an hour past.
        ("3pm (Europe/Berlin)", Some("2026-11-27T14:00:00Z")),
        ("3:30pm (Europe/Berlin)", Some("2026-11-27T14:30:00Z")),
        ("12:30pm (Europe/Berlin)", Some("2026-11-27T11:30:00Z")),
        ("11am (Europe/Berlin)", Some("2026-11-28T10:00:00Z")),
        // Unreadable: the percentage still counts, without a reset.
        ("soon", None),
        ("Nov 31, 8am", None),
        ("Nov 29, 13pm", None),
        ("Nov 29, 8:5am", None),
        ("Smarch 29, 8am", None),
        ("", None),
    ] {
        let text = format!("Current week (all models): 10% used · resets {when}");
        let quota = parse(&text, &now);
        let week = &quota.windows[&QuotaMetric::Week];
        assert_eq!(week.used, 10.0, "{when}");
        assert_eq!(week.reset_at.as_deref(), expected, "{when}");
    }
}

#[test]
fn a_date_without_a_year_rolls_over_into_the_next_one() {
    let line = "Current week (all models): 5% used · resets Jan 2, 9am (Europe/Berlin)";
    let quota = parse(line, &berlin("2026-12-30 22:00"));
    assert_eq!(
        quota.windows[&QuotaMetric::Week].reset_at.as_deref(),
        Some("2027-01-02T08:00:00Z")
    );
    // A reset that passed less than a day ago stays in its own year.
    let line = "Current week (all models): 5% used · resets Dec 31, 11pm (Europe/Berlin)";
    let quota = parse(line, &berlin("2027-01-01 08:00"));
    assert_eq!(
        quota.windows[&QuotaMetric::Week].reset_at.as_deref(),
        Some("2026-12-31T22:00:00Z")
    );
}

#[test]
fn labels_map_to_metrics_and_unknown_ones_are_skipped() {
    let text = "\
Current session: 12.5% used · resets 3pm (Europe/Berlin)
Current week (all models): 40% used
Current week (Sonnet only): 30% used
Current week (FABLE): 20% used
Current week (Haiku): 99% used
Current month: 50% used
Current week (all models): 77% used";
    let quota = parse(text, &berlin("2026-11-27 13:00"));
    let used: Vec<_> = quota.windows.iter().map(|(m, w)| (*m, w.used)).collect();
    assert_eq!(
        used,
        [
            (QuotaMetric::Week, 40.0),
            (QuotaMetric::Session, 12.5),
            (QuotaMetric::Fable, 20.0),
            (QuotaMetric::Sonnet, 30.0),
        ]
    );
    assert_eq!(quota.windows[&QuotaMetric::Week].reset_at, None);
    let five_hour = parse("Current 5-hour limit: 3% used", &berlin("2026-11-27 13:00"));
    assert_eq!(five_hour.windows[&QuotaMetric::Session].used, 3.0);
    let over = parse("Current week: 140% used", &berlin("2026-11-27 13:00"));
    assert_eq!(over.windows[&QuotaMetric::Week].used, 100.0);
}

#[test]
fn text_without_a_known_window_is_unreadable_or_has_no_subscription() {
    let now = berlin("2026-11-27 13:00");
    for text in [
        "",
        "garbage",
        "You are currently using your subscription to power your Claude Code usage",
        "Current week (Haiku): 99% used",
        "Current week (all models): many% used",
        "Current week (all models) 82% used",
        "Current week (all models): 82%",
        "Week (all models): 82% used",
    ] {
        assert_eq!(
            parse(text, &now),
            AgentQuota::unavailable(UNREADABLE),
            "{text}"
        );
    }
    assert_eq!(
        parse("Estimated cost: $1.20", &now),
        AgentQuota::unavailable(NO_SUBSCRIPTION)
    );
}

#[test]
fn the_printed_object_must_be_a_successful_result() {
    let printed = |value: serde_json::Value| value.to_string().into_bytes();
    let ok = printed(json!({"is_error": false, "result": "text", "num_turns": 0}));
    assert_eq!(result_text(&ok).as_deref(), Ok("text"));
    // A byte order mark and a line before the object are tolerated.
    let mut noisy = b"\xEF\xBB\xBFupdate available\r\n".to_vec();
    noisy.extend(&ok);
    noisy.extend(b"\r\n");
    assert_eq!(result_text(&noisy).as_deref(), Ok("text"));
    for (stdout, reason) in [
        (
            printed(json!({"is_error": true, "result": "private material"})),
            CLI_ERROR,
        ),
        (printed(json!({"is_error": false})), UNREADABLE),
        (printed(json!({"result": 42})), UNREADABLE),
        (printed(json!(["result"])), UNREADABLE),
        (b"{\"result\": \"cut".to_vec(), UNREADABLE),
        (b"private material".to_vec(), UNREADABLE),
        (Vec::new(), UNREADABLE),
    ] {
        assert_eq!(result_text(&stdout), Err(reason));
    }
}
