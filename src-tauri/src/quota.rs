//! Read-only CLI subscription limits. Only measured percentages, reset times and display
//! errors cross the IPC boundary.
use crate::models::{AgentId, AppSettings, QuotaMetric};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[cfg(feature = "updates")]
mod live;
#[cfg(feature = "updates")]
mod opencode;
#[cfg(feature = "updates")]
mod read_log;
#[cfg(feature = "updates")]
mod windows;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub used: f64,
    pub reset_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentQuota {
    pub windows: BTreeMap<QuotaMetric, QuotaWindow>,
    pub detail: Option<String>,
    /// Set only when the provider limited the read (HTTP 429, #100): the seconds its
    /// `Retry-After` asks to wait, capped at `MAX_RETRY_AFTER_SECS`; `0` without one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_secs: Option<u64>,
}

/// The longest `Retry-After` taken over from a provider: one day.
pub const MAX_RETRY_AFTER_SECS: u64 = 24 * 60 * 60;
pub const RATE_LIMITED: &str = "Zu viele Abfragen. Bitte später erneut versuchen.";

impl AgentQuota {
    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            windows: BTreeMap::new(),
            detail: Some(detail.into()),
            retry_after_secs: None,
        }
    }

    /// The provider refused the read as too frequent (#100). The panel backs off.
    pub fn rate_limited(retry_after_secs: u64) -> Self {
        Self {
            retry_after_secs: Some(retry_after_secs.min(MAX_RETRY_AFTER_SECS)),
            ..Self::unavailable(RATE_LIMITED)
        }
    }

    /// The agent's read is switched off: a chosen state, not a failure, so no detail.
    pub fn off() -> Self {
        Self {
            windows: BTreeMap::new(),
            detail: None,
            retry_after_secs: None,
        }
    }
}

/// One account's read, gated by the agent's `quotaReads` switch (ADR-0014). With the
/// switch off it returns before `credentials` runs: no path is resolved, no sign-in file
/// or database is opened, no request is built and nothing is logged. `profile` (`None` =
/// `default`) only names the read in the log.
pub fn read(
    agent: AgentId,
    profile: Option<&str>,
    settings: &AppSettings,
    credentials: impl FnOnce() -> Result<PathBuf, &'static str>,
) -> AgentQuota {
    let profile = profile.unwrap_or(crate::models::DEFAULT_PROFILE);
    read_with(agent, settings, credentials, |agent, path| {
        fetch(agent, profile, path)
    })
}

fn read_with(
    agent: AgentId,
    settings: &AppSettings,
    credentials: impl FnOnce() -> Result<PathBuf, &'static str>,
    fetch: impl FnOnce(AgentId, &Path) -> AgentQuota,
) -> AgentQuota {
    if !*settings.quota_reads.get(agent) {
        return AgentQuota::off();
    }
    match credentials() {
        Ok(path) => fetch(agent, &path),
        Err(detail) => AgentQuota::unavailable(detail),
    }
}

/// One provider request, logged (#109): a debug line for every read, an info line when the
/// provider refused or limited it or could not be reached.
#[cfg(feature = "updates")]
fn fetch(agent: AgentId, profile: &str, credentials_path: &Path) -> AgentQuota {
    let started = std::time::Instant::now();
    let mut exchange = read_log::Exchange::default();
    let result = live::fetch(agent, credentials_path, &mut exchange);
    let (debug, info) = read_log::lines(agent, profile, &exchange, &result, started.elapsed());
    crate::app_log::debug(|| debug);
    if let Some(info) = info {
        crate::app_log::info(&info);
    }
    result.unwrap_or_else(AgentQuota::unavailable)
}

#[cfg(not(feature = "updates"))]
fn fetch(_: AgentId, _: &str, _: &Path) -> AgentQuota {
    AgentQuota::unavailable("Dieses Build unterstützt keine Kontingentabfragen.")
}

#[cfg(test)]
mod gate_tests;
#[cfg(all(test, feature = "updates"))]
mod tests;
