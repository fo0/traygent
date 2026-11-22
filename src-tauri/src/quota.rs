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
}

impl AgentQuota {
    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            windows: BTreeMap::new(),
            detail: Some(detail.into()),
        }
    }

    /// The agent's read is switched off: a chosen state, not a failure, so no detail.
    pub fn off() -> Self {
        Self {
            windows: BTreeMap::new(),
            detail: None,
        }
    }
}

/// One account's read, gated by the agent's `quotaReads` switch (ADR-0014). With the
/// switch off it returns before `credentials` runs: no path is resolved, no sign-in file
/// or database is opened and no request is built.
pub fn read(
    agent: AgentId,
    settings: &AppSettings,
    credentials: impl FnOnce() -> Result<PathBuf, &'static str>,
) -> AgentQuota {
    read_with(agent, settings, credentials, fetch)
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

#[cfg(feature = "updates")]
pub fn fetch(agent: AgentId, credentials_path: &Path) -> AgentQuota {
    live::fetch(agent, credentials_path).unwrap_or_else(AgentQuota::unavailable)
}

#[cfg(not(feature = "updates"))]
pub fn fetch(_: AgentId, _: &Path) -> AgentQuota {
    AgentQuota::unavailable("Dieses Build unterstützt keine Kontingentabfragen.")
}

#[cfg(test)]
mod gate_tests;
#[cfg(all(test, feature = "updates"))]
mod tests;
