use super::{AgentQuota, QuotaWindow};
use crate::models::{AgentId, QuotaMetric};
use chrono::{DateTime, NaiveDateTime, SecondsFormat, TimeDelta, Utc};
use serde_json::Value;
use std::collections::BTreeMap;

type Windows = BTreeMap<QuotaMetric, QuotaWindow>;

pub(super) fn parse(
    agent: AgentId,
    body: &Value,
    codex_used_headers: [Option<f64>; 2],
    now: DateTime<Utc>,
) -> AgentQuota {
    use QuotaMetric::*;
    let mut windows = Windows::new();
    match agent {
        AgentId::Claude => {
            for (metric, field) in [
                (Week, "seven_day"),
                (Session, "five_hour"),
                (Sonnet, "seven_day_sonnet"),
            ] {
                insert_window(
                    &mut windows,
                    metric,
                    &body[field],
                    "utilization",
                    "resets_at",
                );
            }
            if let Some(limits) = body["limits"].as_array() {
                for limit in limits {
                    if limit["kind"] == "weekly_scoped"
                        && limit
                            .pointer("/scope/model/display_name")
                            .and_then(Value::as_str)
                            == Some("Fable")
                    {
                        insert_window(&mut windows, Fable, limit, "percent", "resets_at");
                    }
                }
            }
        }
        AgentId::Codex => {
            codex_windows(
                &mut windows,
                &body["rate_limit"],
                [Session, Week],
                codex_used_headers,
                now,
            );
            if let Some(limits) = body["additional_rate_limits"].as_array() {
                for entry in limits {
                    let is_spark = ["limit_name", "metered_feature"].iter().any(|field| {
                        entry[field]
                            .as_str()
                            .is_some_and(|s| s.to_ascii_lowercase().contains("spark"))
                    });
                    if is_spark {
                        codex_windows(
                            &mut windows,
                            &entry["rate_limit"],
                            [Spark, SparkWeek],
                            [None; 2],
                            now,
                        );
                    }
                }
            }
        }
        AgentId::Opencode => {
            for (metric, field) in [(Week, "weekly"), (Session, "rolling"), (Month, "monthly")] {
                insert_window(
                    &mut windows,
                    metric,
                    &body["usage"][field],
                    "percent",
                    "resetsAt",
                );
            }
        }
    }
    if windows.is_empty() {
        return AgentQuota::unavailable("Der Anbieter meldet keine Kontingente für dieses Konto.");
    }
    AgentQuota {
        windows,
        detail: None,
    }
}

fn insert_window(
    windows: &mut Windows,
    metric: QuotaMetric,
    entry: &Value,
    used: &str,
    reset: &str,
) {
    if let Some(window) = window(number(&entry[used]), reset_time(&entry[reset])) {
        windows.insert(metric, window);
    }
}

fn codex_windows(
    windows: &mut Windows,
    limits: &Value,
    metrics: [QuotaMetric; 2],
    fallback: [Option<f64>; 2],
    now: DateTime<Utc>,
) {
    // A known duration wins over an inferred one, even in the secondary slot.
    // Weekly-only plans can put their seven-day window in the primary slot.
    let mut entries = [
        (0, &limits["primary_window"]),
        (1, &limits["secondary_window"]),
    ];
    entries.sort_by_key(|(_, entry)| entry["limit_window_seconds"].is_null());
    for (slot, entry) in entries {
        let duration = &entry["limit_window_seconds"];
        let metric = match number(duration) {
            Some(18_000.0) => metrics[0],
            Some(604_800.0) => metrics[1],
            None if duration.is_null() => metrics[slot],
            _ => continue,
        };
        let used = number(&entry["used_percent"]).or(fallback[slot]);
        let reset = reset_time(&entry["reset_at"]).or_else(|| {
            let seconds =
                number(&entry["reset_after_seconds"]).filter(|seconds| *seconds >= 0.0)?;
            let delay = TimeDelta::try_milliseconds((seconds * 1000.0).round() as i64)?;
            now.checked_add_signed(delay)
        });
        if let Some(window) = window(used, reset) {
            windows.entry(metric).or_insert(window);
        }
    }
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|n| n.is_finite())
}

fn window(used: Option<f64>, reset: Option<DateTime<Utc>>) -> Option<QuotaWindow> {
    let used = used.filter(|used| used.is_finite() && *used >= 0.0)?;
    Some(QuotaWindow {
        used: used.min(100.0),
        reset_at: reset.map(|time| time.to_rfc3339_opts(SecondsFormat::AutoSi, true)),
    })
}

fn reset_time(value: &Value) -> Option<DateTime<Utc>> {
    if let Some(text) = value.as_str() {
        let text = text.trim();
        if let Ok(date) = DateTime::parse_from_rfc3339(text) {
            return Some(date.to_utc());
        }
        if let Ok(date) = NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f") {
            return Some(date.and_utc());
        }
    }
    let epoch = number(value)?;
    let millis = if epoch.abs() >= 10_000_000_000.0 {
        epoch
    } else {
        epoch * 1000.0
    };
    DateTime::from_timestamp_millis(millis.round() as i64)
}
