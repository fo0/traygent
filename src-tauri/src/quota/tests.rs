use super::*;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

const RESET: &str = "2026-02-01T12:00:00Z";

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(RESET).unwrap().to_utc()
}

fn parse(agent: AgentId, body: Value) -> Value {
    serde_json::to_value(windows::parse(agent, &body, [None; 2], now())).unwrap()
}

#[test]
fn claude_reports_real_zero_and_only_the_model_windows_present() {
    let quota = parse(
        AgentId::Claude,
        json!({
            "seven_day": {"utilization": 0, "resets_at": RESET},
            "five_hour": {"utilization": "15.5", "resets_at": "invalid"},
            "seven_day_sonnet": {"utilization": 27},
            "limits": [
                {"kind": "weekly_scoped", "scope": {"model": {"display_name": "Other"}}, "percent": 70},
                {"kind": "weekly_scoped", "scope": {"model": {"display_name": "Fable"}}, "percent": 60}
            ]
        }),
    );
    assert_eq!(
        quota,
        json!({"windows":{
        "week": {"used": 0.0, "resetAt": RESET},
        "session": {"used": 15.5, "resetAt": null},
        "sonnet": {"used": 27.0, "resetAt": null},
        "fable": {"used": 60.0, "resetAt": null}
    }, "detail": null})
    );
    let partial = parse(AgentId::Claude, json!({"five_hour":{"utilization": 5}}));
    assert_eq!(partial["windows"].as_object().unwrap().len(), 1);
    assert!(partial["windows"].get("week").is_none());
}

#[test]
fn codex_weekly_only_and_spark_limits_use_the_period_and_normalize_resets() {
    let quota = parse(
        AgentId::Codex,
        json!({
            "rate_limit": {
                "primary_window":{"used_percent": 30, "limit_window_seconds": 604800, "reset_at": now().timestamp()}
            },
            "additional_rate_limits":[
                {"limit_name":"other", "rate_limit":{"primary_window":{"used_percent": 80}}},
                {"metered_feature":"codex_spark_preview", "rate_limit":{
                "primary_window":{"used_percent": 75, "limit_window_seconds": 18000, "reset_after_seconds": "300.5"},
                    "secondary_window":{"used_percent": 10, "limit_window_seconds": 604800}
                }}
            ]
        }),
    );
    assert_eq!(
        quota["windows"],
        json!({
            "week": {"used": 30.0, "resetAt": RESET},
            "spark": {"used": 75.0, "resetAt": "2026-02-01T12:05:00.500Z"},
            "sparkWeek": {"used": 10.0, "resetAt": null}
        })
    );
}

#[test]
fn codex_percent_headers_fill_missing_body_values() {
    let data = json!({"rate_limit":{
        "primary_window":{"used_percent": 13},
        "secondary_window":{"reset_at": now().timestamp_millis()}
    }});
    let quota = windows::parse(AgentId::Codex, &data, [Some(99.0), Some(21.0)], now());
    assert_eq!(quota.windows[&QuotaMetric::Session].used, 13.0);
    assert_eq!(quota.windows[&QuotaMetric::Week].used, 21.0);
    assert_eq!(
        quota.windows[&QuotaMetric::Week].reset_at.as_deref(),
        Some(RESET)
    );
}

#[test]
fn codex_known_duration_precedes_an_inferred_slot() {
    let quota = parse(
        AgentId::Codex,
        json!({"rate_limit":{
            "primary_window": {"used_percent": 10},
            "secondary_window": {"used_percent": 20, "limit_window_seconds": 18000}
        }}),
    );
    assert_eq!(
        quota["windows"],
        json!({"session": {"used": 20.0, "resetAt": null}})
    );
}

#[test]
fn opencode_rolling_week_and_month_are_independent() {
    let quota = parse(
        AgentId::Opencode,
        json!({"usage":{
            "rolling":{"percent": 4, "resetsAt": RESET},
            "weekly":{"percent": 100, "resetsAt": "2026-02-01T13:00:00+01:00"},
            "monthly":{"percent": 8, "resetsAt": null}
        }}),
    );
    assert_eq!(
        quota["windows"],
        json!({
            "session":{"used": 4.0, "resetAt": RESET},
            "week":{"used": 100.0, "resetAt": RESET},
            "month":{"used": 8.0, "resetAt": null}
        })
    );
    let partial = parse(
        AgentId::Opencode,
        json!({"usage":{"monthly":{"percent": 7}}}),
    );
    assert_eq!(partial["windows"].as_object().unwrap().len(), 1);
    assert!(partial["windows"].get("week").is_none());
}

#[test]
fn unusable_measurements_never_become_zero_or_expose_provider_text() {
    for agent in AgentId::ALL {
        let quota = parse(
            agent,
            json!({
                "error":"private material",
                "seven_day":{"utilization":"NaN"},
                "usage":{"weekly":{"percent":-1}},
                "rate_limit":{"primary_window":{"used_percent":14,"limit_window_seconds":86400}}
            }),
        );
        assert_eq!(quota["windows"], json!({}));
        assert!(quota["detail"].is_string());
        assert!(!quota.to_string().contains("private material"));
    }
}
