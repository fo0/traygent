//! The answer of Claude Code's `/usage` (#110): `claude -p /usage --output-format json`
//! prints one JSON object whose `result` is the text the interactive `/usage` shows.
//!
//! That text is no contract. Only lines `Current <label>: <N>% used` with an optional
//! ` · resets <when>` count; everything else — the "What's contributing" block too — is
//! ignored and may change freely. The shapes seen are pinned in `fixtures/claude-usage/`.
use super::{AgentQuota, QuotaWindow};
use crate::models::QuotaMetric;
use crate::profiles::files::without_bom;
use chrono::{
    DateTime, Datelike, Days, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, SecondsFormat,
    TimeDelta, TimeZone, Utc,
};
use serde::Deserialize;
use std::collections::BTreeMap;

pub(super) const UNREADABLE: &str = "Die Ausgabe der Claude-Code-CLI ist nicht lesbar.";
pub(super) const NO_SUBSCRIPTION: &str =
    "Die Claude-Code-CLI meldet kein Abo-Kontingent für diese Anmeldung.";
pub(super) const CLI_ERROR: &str = "Die Claude-Code-CLI meldet einen Fehler.";
const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

#[derive(Deserialize)]
struct Printed {
    #[serde(default)]
    is_error: bool,
    result: Option<String>,
    /// `usage` when the CLI answered `/usage` itself; without it the text came from
    /// somewhere else (a model turn), whatever it says.
    local_command: Option<String>,
}

/// The `result` text of the printed JSON object. A line of other output before it is
/// skipped; an error the CLI reports never passes its text on, and only the answer of the
/// local `/usage` command counts.
pub(super) fn result_text(stdout: &[u8]) -> Result<String, &'static str> {
    let text = String::from_utf8_lossy(without_bom(stdout));
    let printed = serde_json::from_str::<Printed>(text.trim())
        .ok()
        .or_else(|| {
            text.lines()
                .rev()
                .find_map(|line| serde_json::from_str::<Printed>(line.trim()).ok())
        })
        .ok_or(UNREADABLE)?;
    if printed.is_error {
        return Err(CLI_ERROR);
    }
    if printed.local_command.as_deref() != Some("usage") {
        return Err(UNREADABLE);
    }
    printed.result.ok_or(UNREADABLE)
}

/// The windows of a `/usage` text. `now` carries the local zone: the CLI prints its reset
/// times in the machine's zone, so the zone it names in parentheses is taken to be that
/// ([`reset_at`]).
pub(super) fn parse<Tz: TimeZone>(text: &str, now: &DateTime<Tz>) -> AgentQuota {
    let zone = now.timezone();
    let resolve = |at: NaiveDateTime| {
        zone.from_local_datetime(&at)
            .map(|at| at.with_timezone(&Utc))
    };
    let (utc, local) = (now.with_timezone(&Utc), now.naive_local());
    let mut windows = BTreeMap::new();
    let mut usage_lines = 0;
    for line in text.lines() {
        let Some((label, used, when)) = usage_line(line) else {
            continue;
        };
        usage_lines += 1;
        let Some(metric) = metric(label) else {
            continue;
        };
        let reset_at = when
            .and_then(|when| reset_at(when, utc, local, resolve))
            .map(|at| at.to_rfc3339_opts(SecondsFormat::AutoSi, true));
        windows.entry(metric).or_insert(QuotaWindow {
            used: used.min(100.0),
            reset_at,
        });
    }
    if !windows.is_empty() {
        return AgentQuota {
            windows,
            detail: None,
            retry_after_secs: None,
        };
    }
    // An API-key or signed-out run prints its session cost instead of subscription lines.
    if usage_lines == 0 && text.contains("Estimated cost") {
        return AgentQuota::unavailable(NO_SUBSCRIPTION);
    }
    AgentQuota::unavailable(UNREADABLE)
}

/// `Current <label>: <N>% used[ · resets <when>]` as label, percentage and `<when>`.
fn usage_line(line: &str) -> Option<(&str, f64, Option<&str>)> {
    let line = line.trim();
    let rest = line
        .get(..8)
        .filter(|head| head.eq_ignore_ascii_case("current "))
        .map(|_| &line[8..])?;
    let (label, rest) = rest.split_once(':')?;
    let rest = rest.trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    let used = rest[..end]
        .parse::<f64>()
        .ok()
        .filter(|used| used.is_finite())?;
    let rest = rest[end..].strip_prefix('%')?.trim_start();
    let rest = rest
        .get(..4)
        .filter(|word| word.eq_ignore_ascii_case("used"))
        .map(|_| &rest[4..])?;
    let when = rest
        .to_ascii_lowercase()
        .find("resets ")
        .map(|at| rest[at + "resets ".len()..].trim())
        .filter(|when| !when.is_empty());
    Some((label.trim(), used, when))
}

/// `week (all models)` → week, `week (Fable)` → Fable, `week (Sonnet …)` → Sonnet, `session`
/// or `5-hour …` → session; any other label is skipped.
fn metric(label: &str) -> Option<QuotaMetric> {
    let label = label.to_lowercase();
    let (head, model) = match label.split_once('(') {
        Some((head, model)) => (head.trim(), Some(model.trim_end_matches(')').trim())),
        None => (label.trim(), None),
    };
    match head {
        "week" => match model {
            None | Some("all models") => Some(QuotaMetric::Week),
            Some(model) if model.contains("fable") => Some(QuotaMetric::Fable),
            Some(model) if model.contains("sonnet") => Some(QuotaMetric::Sonnet),
            Some(_) => None,
        },
        "session" => Some(QuotaMetric::Session),
        head if head.starts_with("5-hour") || head.starts_with("5 hour") => {
            Some(QuotaMetric::Session)
        }
        _ => None,
    }
}

/// `Nov 29, 7:59am (Europe/Berlin)`, `Nov 29, 8am (…)` or a time alone, `3pm (…)`, in the
/// local zone that `resolve` applies (`now`, and `local`: now in that zone). No year is printed: a date is the
/// first one at or after now − 1 day; a time alone is today's while it is at most an hour
/// past, else tomorrow's. A chosen time the zone skips (a clock change) gives no reset: the
/// next candidate would be a year or a day off.
///
/// The zone printed in parentheses is not compared with the machine's: Windows names its
/// zones differently (`W. Europe Standard Time`) and the mapping would need a new dependency,
/// so the CLI is taken to print in the machine's zone, as it does.
fn reset_at(
    when: &str,
    now: DateTime<Utc>,
    local: NaiveDateTime,
    resolve: impl Fn(NaiveDateTime) -> LocalResult<DateTime<Utc>> + Copy,
) -> Option<DateTime<Utc>> {
    let when = when.rfind('(').map_or(when, |at| &when[..at]).trim();
    let (candidates, back): (Vec<NaiveDateTime>, TimeDelta) = match when.split_once(',') {
        Some((date, time)) => {
            let (month, day) = month_day(date)?;
            let time = clock(time)?;
            let dates = (local.year() - 1..=local.year() + 1)
                .filter_map(|year| NaiveDate::from_ymd_opt(year, month, day))
                .map(|date| date.and_time(time));
            (dates.collect(), TimeDelta::days(1))
        }
        None => {
            let time = clock(when)?;
            let dates = [Days::new(0), Days::new(1)]
                .into_iter()
                .filter_map(|days| local.date().checked_add_days(days))
                .map(|date| date.and_time(time));
            (dates.collect(), TimeDelta::hours(1))
        }
    };
    let (floor, floor_local) = (now - back, local - back);
    for at in candidates {
        match resolve(at).earliest() {
            Some(utc) if utc >= floor => return Some(utc),
            Some(_) => {}
            // The zone skips this time: when it is the one meant, there is no reset.
            None if at >= floor_local => return None,
            None => {}
        }
    }
    None
}

/// `Nov 29` as month and day; the month by its first three letters.
fn month_day(text: &str) -> Option<(u32, u32)> {
    let mut words = text.split_whitespace();
    let month = words.next()?.to_lowercase();
    let month = MONTHS.iter().position(|name| month.starts_with(name))? as u32 + 1;
    let day = words.next()?.parse::<u32>().ok()?;
    words.next().is_none().then_some((month, day))
}

/// `7:59am`, `8am`, `12pm` or a 24-hour `15:30`.
fn clock(text: &str) -> Option<NaiveTime> {
    let text = text.trim().to_ascii_lowercase().replace(' ', "");
    let (digits, half) = match (text.strip_suffix("am"), text.strip_suffix("pm")) {
        (Some(digits), _) => (digits, Some(0)),
        (_, Some(digits)) => (digits, Some(12)),
        _ => (text.as_str(), None),
    };
    let (hour, minute) = match digits.split_once(':') {
        Some((hour, minute)) if minute.len() == 2 => (hour, minute.parse::<u32>().ok()?),
        Some(_) => return None,
        None => (digits, 0),
    };
    let hour = hour.parse::<u32>().ok()?;
    let hour = match half {
        Some(offset) if (1..=12).contains(&hour) => hour % 12 + offset,
        Some(_) => return None,
        None => hour,
    };
    NaiveTime::from_hms_opt(hour, minute, 0)
}

#[cfg(test)]
mod tests;
