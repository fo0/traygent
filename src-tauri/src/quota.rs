//! Read-only CLI subscription limits. Only measured percentages, reset times and display
//! errors cross the IPC boundary.
use crate::models::{AgentId, QuotaMetric};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

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
}

#[cfg(feature = "updates")]
pub fn fetch(agent: AgentId, credentials_path: &Path) -> AgentQuota {
    live::fetch(agent, credentials_path).unwrap_or_else(AgentQuota::unavailable)
}

#[cfg(not(feature = "updates"))]
pub fn fetch(_: AgentId, _: &Path) -> AgentQuota {
    AgentQuota::unavailable("Dieses Build unterstützt keine Kontingentabfragen.")
}

#[cfg(all(test, feature = "updates"))]
mod tests;
