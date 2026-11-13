//! Agent profiles in the service (#33): the statuses the snapshot shows and the sync runs.
//! The file system work is a [`SyncPlan`], so the shell can run it without the service lock.
use super::Service;
use crate::models::{AgentId, EnvId, LaunchRequest, ProfileStatus};
use crate::profiles::status::{self, Facts, Key};
use crate::profiles::{recipe, Homes, Linker, ProfileReport, SyncOutcome, SyncPlan};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Leads `LaunchOutcome.warning`; the session started all the same.
pub const SYNC_WARNING: &str = "Profile konnten nicht vollständig synchronisiert werden: ";
const PROFILE_DIR_FAILED: &str = "Der Profilordner konnte nicht angelegt werden.";
const MAX_ERROR_CHARS: usize = 400;

/// In memory only; read again at every detection, sync and settings save.
#[derive(Default)]
pub(super) struct ProfileState {
    homes: Option<Homes>,
    facts: HashMap<Key, Facts>,
    /// What the last sync of a profile could not do.
    errors: HashMap<Key, String>,
    /// The sync after the first detection has run.
    synced_at_startup: bool,
}

fn capped(text: String) -> String {
    if text.chars().count() <= MAX_ERROR_CHARS {
        return text;
    }
    let mut short: String = text.chars().take(MAX_ERROR_CHARS).collect();
    short.push('…');
    short
}

/// `<label>: <error> · <error>` per profile with errors, joined by `; `.
fn warning(reports: &[(AgentId, Vec<ProfileReport>)]) -> Option<String> {
    let failed: Vec<String> = reports
        .iter()
        .flat_map(|(_, reports)| reports)
        .filter(|report| !report.errors.is_empty())
        .map(|report| format!("{}: {}", report.label, report.errors.join(" · ")))
        .collect();
    (!failed.is_empty()).then(|| capped(format!("{SYNC_WARNING}{}", failed.join("; "))))
}

impl Service {
    /// Where profiles live, resolved from the host once at startup.
    pub fn set_profile_homes(&mut self, homes: Option<Homes>) {
        self.profiles.homes = homes;
        self.touch();
    }

    pub(super) fn profile_statuses(&self) -> Vec<ProfileStatus> {
        status::list(
            self.profiles.homes.as_ref(),
            &self.state.settings,
            &self.profiles.facts,
            &self.profiles.errors,
        )
    }

    /// Every agent with profiles and its current list; run it without the service lock.
    pub fn profile_plan(&self) -> SyncPlan {
        SyncPlan::of(self.profiles.homes.as_ref(), &self.state.settings)
    }

    /// True once: the detection that finishes first also syncs the profiles.
    pub fn take_startup_profile_sync(&mut self) -> bool {
        !std::mem::replace(&mut self.profiles.synced_at_startup, true)
    }

    /// Commits a finished [`SyncPlan`]: the facts of the profiles it read and, after a sync,
    /// each synced profile's error (or none). Returns the warning a start shows.
    pub fn apply_profiles(&mut self, outcome: SyncOutcome) -> Option<String> {
        let state = &mut self.profiles;
        state.facts.extend(outcome.facts);
        let mut text = None;
        if let Some(reports) = &outcome.reports {
            for (agent, reports) in reports {
                state.errors.retain(|(listed, _), _| listed != agent);
                for report in reports.iter().filter(|report| !report.errors.is_empty()) {
                    let error = capped(report.errors.join(" · "));
                    state.errors.insert((*agent, report.id.clone()), error);
                }
            }
            text = warning(reports);
        }
        // A removed profile leaves nothing behind in memory (its directory stays on disk).
        let settings = &self.state.settings;
        let listed = |(agent, id): &Key| {
            id == crate::models::DEFAULT_PROFILE
                || settings.profiles.get(*agent).iter().any(|p| &p.id == id)
        };
        state.facts.retain(|key, _| listed(key));
        state.errors.retain(|key, _| listed(key));
        self.touch();
        text
    }

    /// Reads every profile's status again (after a settings save, a sign-in).
    pub fn refresh_profiles(&mut self) {
        let outcome = self.profile_plan().read();
        self.apply_profiles(outcome);
    }

    /// Syncs every profile of every agent now; the warning names what could not be done.
    pub fn sync_profiles(&mut self, linker: &dyn Linker, now: DateTime<Utc>) -> Option<String> {
        let outcome = self.profile_plan().run(linker, now);
        self.apply_profiles(outcome)
    }

    /// The sync before a local start of an agent that has extra profiles: all of that
    /// agent's profiles. A failure is a warning — the session still starts — except when the
    /// requested profile's directory cannot be created. `request` is the prepared one.
    pub fn sync_for_launch(
        &mut self,
        request: &LaunchRequest,
        linker: &dyn Linker,
        now: DateTime<Utc>,
    ) -> Result<Option<String>, String> {
        let agent = request.agent;
        let extra = !self.state.settings.profiles.get(agent).is_empty();
        if request.env != EnvId::Local || recipe(agent).is_none() || !extra {
            return Ok(None);
        }
        let outcome = self.profile_plan().only(agent).run(linker, now);
        let blocked = outcome.reports.iter().flatten().any(|(_, reports)| {
            reports
                .iter()
                .any(|report| report.dir_failed && Some(&report.id) == request.profile.as_ref())
        });
        let text = self.apply_profiles(outcome);
        if blocked {
            return Err(PROFILE_DIR_FAILED.into());
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests;
