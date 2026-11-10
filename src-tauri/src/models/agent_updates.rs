//! The agent-update part of the IPC data model (#16), mirrored from `src/lib/agentUpdateTypes.ts`.
use super::AgentId;
use serde::{Deserialize, Serialize};

/// Where an agent's last update run stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentUpdateState {
    /// The run works on this agent right now; a local start is refused meanwhile.
    Running,
    /// The update command installed another version.
    Updated,
    /// The update command succeeded and the version stayed.
    Current,
    /// A session (or an unreadable process list) kept the update from running.
    InUse,
    Failed,
}

/// The last update run of one agent's local CLI since the app started.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentUpdateStatus {
    pub agent: AgentId,
    pub state: AgentUpdateState,
    /// The CLI version after the run, when known (`updated`, `current`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The version before the run; only for `updated`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// RFC 3339 time the state was reached.
    pub at: String,
}
