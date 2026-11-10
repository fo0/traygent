//! The agent update runs (#16): which run is in progress and each agent's last status.
//!
//! The process list and the update commands run in [`crate::agent_updates`] without the
//! service; these methods only commit their results, so the shell never holds the service
//! lock on a process. `updating` dedupes a run, every change bumps `revision`.
use super::Service;
use crate::agent_updates;
use crate::detect::Candidate;
use crate::models::{AgentId, AgentUpdateState, AgentUpdateStatus};
use std::time::Instant;

/// `update_agents` while a run is in progress.
pub const UPDATE_RUNNING: &str = "Agenten werden bereits aktualisiert.";
/// `update_agents` before the local CLIs are known.
pub const UPDATE_DETECTING: &str =
    "Die Agenten werden noch gesucht. Bitte gleich noch einmal versuchen.";

impl Service {
    /// The ready local CLIs a run updates, in catalog order; `None` while a detection runs
    /// or before the first one finished (the caller retries later).
    pub fn agent_update_targets(&self) -> Option<Vec<(AgentId, Candidate)>> {
        let detection = self.detection.as_ref().filter(|_| !self.detecting)?;
        let ready = AgentId::ALL
            .into_iter()
            .filter_map(|agent| Some((agent, detection.clis.get(agent).clone()?)))
            .collect();
        Some(ready)
    }

    /// Marks an update run as in progress; `false` while one already is.
    pub fn begin_agent_updates(&mut self) -> bool {
        if std::mem::replace(&mut self.updating, true) {
            return false;
        }
        self.touch();
        true
    }

    /// Begins a run over the ready local CLIs for the `update_agents` command.
    pub fn start_agent_updates(&mut self) -> Result<Vec<(AgentId, Candidate)>, String> {
        if self.updating {
            return Err(UPDATE_RUNNING.into());
        }
        let targets = self.agent_update_targets().ok_or(UPDATE_DETECTING)?;
        self.begin_agent_updates();
        Ok(targets)
    }

    /// Replaces the agent's status with the one a run just reached.
    pub fn set_agent_update(&mut self, status: AgentUpdateStatus) {
        let agent = status.agent;
        *self.agent_updates.get_mut(agent) = Some(status);
        self.touch();
    }

    /// True while a run works on this agent's local CLI.
    pub fn agent_updating(&self, agent: AgentId) -> bool {
        let status = self.agent_updates.get(agent).as_ref();
        status.is_some_and(|status| status.state == AgentUpdateState::Running)
    }

    /// Ends the run. A status still `running` belongs to a run that died (a panic): it ends
    /// as `failed`, so no start stays refused.
    pub fn finish_agent_updates(&mut self, now: Instant) {
        self.updating = false;
        self.last_update_run = Some(now);
        for agent in AgentId::ALL {
            if let Some(status) = self.agent_updates.get_mut(agent) {
                if status.state == AgentUpdateState::Running {
                    status.state = AgentUpdateState::Failed;
                    status.at = crate::recents::timestamp(chrono::Utc::now());
                }
            }
        }
        self.touch();
    }

    /// Whether the background schedule should start a run now: the setting is on, none is
    /// in progress and [`agent_updates::due`] agrees. `started` is the app's start.
    pub fn agent_updates_due(&self, started: Instant, now: Instant) -> bool {
        let since = |earlier: Instant| now.saturating_duration_since(earlier);
        !self.updating
            && agent_updates::due(
                self.state.settings.agent_updates,
                since(started),
                self.last_update_run.map(since),
            )
    }

    /// The statuses for the snapshot, in catalog order.
    pub(super) fn agent_update_statuses(&self) -> Vec<AgentUpdateStatus> {
        AgentId::ALL
            .into_iter()
            .filter_map(|agent| self.agent_updates.get(agent).clone())
            .collect()
    }
}

#[cfg(test)]
mod tests;
