//! Agent profiles in the service (#33): the statuses the snapshot shows and the sync runs.
//! The file system work is a [`SyncPlan`], so the shell can run it without the service lock.
use super::Service;
use crate::models::{AgentId, EnvId, LaunchRequest, ProfileStatus};
use crate::profiles::status::{self, Facts, Key};
use crate::profiles::{recipe, Homes, Linker, ProfileReport, SyncOutcome, SyncPlan};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// `LaunchOutcome.warning`: one sentence, the session started all the same. What could not
/// be done stays in each profile's `syncError`.
pub const SYNC_WARNING: &str = "Profile konnten nicht vollständig synchronisiert werden.";
const PROFILE_DIR_FAILED: &str = "Der Profilordner konnte nicht angelegt werden.";
const MAX_ERROR_CHARS: usize = 400;
/// How long after the last sync of any kind the background schedule syncs the profiles
/// again (#37); before the first one the interval runs from the app's start.
pub const PROFILE_SYNC_INTERVAL: Duration = Duration::from_secs(15 * 60);
/// How often the background schedule asks whether a sync is due.
pub const PROFILE_SYNC_TICK: Duration = Duration::from_secs(60);

/// In memory only; read again at every detection, sync and settings save.
#[derive(Default)]
pub(super) struct ProfileState {
    homes: Option<Homes>,
    facts: HashMap<Key, Facts>,
    /// What the last sync of a profile could not do.
    errors: HashMap<Key, String>,
    /// The sync after the first detection has run.
    synced_at_startup: bool,
    /// When the last sync of any kind finished (or a background one broke down). One clock
    /// for all agents: a start's sync of one agent restarts it for every agent, which is
    /// right only while Claude Code is the one agent with a recipe — revisit (one clock per
    /// agent) when a second recipe arrives.
    last_sync: Option<Instant>,
}

fn capped(text: String) -> String {
    if text.chars().count() <= MAX_ERROR_CHARS {
        return text;
    }
    let mut short: String = text.chars().take(MAX_ERROR_CHARS).collect();
    short.push('…');
    short
}

fn warning(reports: &[(AgentId, Vec<ProfileReport>)]) -> Option<String> {
    let failed = reports
        .iter()
        .flat_map(|(_, reports)| reports)
        .any(|report| !report.errors.is_empty());
    failed.then(|| SYNC_WARNING.to_string())
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
            // Every sync — at startup, after a save, on demand, before a start, in the
            // background — restarts the background schedule's interval.
            state.last_sync = Some(Instant::now());
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

    /// Whether the background schedule syncs the profiles now: some agent has an extra
    /// profile, the profile homes are known and [`PROFILE_SYNC_INTERVAL`] has passed since
    /// the last sync finished — before the first one, since `started` (the app's start).
    /// A running agent update does not hold it back: a sync never runs the agent's CLI.
    pub fn profile_sync_due(&self, started: Instant, now: Instant) -> bool {
        let settings = &self.state.settings;
        let extra = AgentId::ALL
            .into_iter()
            .any(|agent| recipe(agent).is_some() && !settings.profiles.get(agent).is_empty());
        let last = self.profiles.last_sync.unwrap_or(started);
        extra
            && self.profiles.homes.is_some()
            && now.saturating_duration_since(last) >= PROFILE_SYNC_INTERVAL
    }

    /// A background sync that broke down (its worker panicked) still restarts the interval,
    /// so the schedule does not retry it every tick.
    pub fn skip_profile_sync(&mut self, now: Instant) {
        self.profiles.last_sync = Some(now);
    }

    /// Reads every profile's status again, the service borrowed throughout (tests; the
    /// shell runs the plan without the lock).
    pub fn refresh_profiles(&mut self) {
        let outcome = self.profile_plan().read();
        self.apply_profiles(outcome);
    }

    /// Syncs every profile of every agent now, the service borrowed throughout (tests; the
    /// shell runs the plan without the lock).
    pub fn sync_profiles(&mut self, linker: &dyn Linker, now: DateTime<Utc>) -> Option<String> {
        let outcome = self.profile_plan().run(linker, now);
        self.apply_profiles(outcome)
    }

    /// The sync before a local start of an agent that has extra profiles: all of that
    /// agent's profiles, `None` when there is nothing to sync. `request` is the prepared
    /// one. Run the plan without the service lock, then [`Service::finish_launch_sync`].
    pub fn launch_sync_plan(&self, request: &LaunchRequest) -> Option<SyncPlan> {
        let agent = request.agent;
        let extra = !self.state.settings.profiles.get(agent).is_empty();
        (request.env == EnvId::Local && recipe(agent).is_some() && extra)
            .then(|| self.profile_plan().only(agent))
    }

    /// Commits the sync of a start. A failure is a warning — the session still starts —
    /// except when the requested profile's directory cannot be used.
    pub fn finish_launch_sync(
        &mut self,
        request: &LaunchRequest,
        outcome: SyncOutcome,
    ) -> Result<Option<String>, String> {
        let blocked = outcome
            .reports
            .iter()
            .flatten()
            .flat_map(|(_, reports)| reports)
            .find(|report| report.dir_failed && Some(&report.id) == request.profile.as_ref())
            .map(|report| report.foreign);
        let text = self.apply_profiles(outcome);
        match blocked {
            Some(true) => Err(format!(
                "Der {}.",
                crate::profiles::sync::foreign_dir(request.agent)
            )),
            Some(false) => Err(PROFILE_DIR_FAILED.into()),
            None => Ok(text),
        }
    }

    /// Plan, run and commit in one call, the service borrowed throughout (tests; the shell
    /// runs the plan without the lock).
    pub fn sync_for_launch(
        &mut self,
        request: &LaunchRequest,
        linker: &dyn Linker,
        now: DateTime<Utc>,
    ) -> Result<Option<String>, String> {
        let Some(plan) = self.launch_sync_plan(request) else {
            return Ok(None);
        };
        self.finish_launch_sync(request, plan.run(linker, now))
    }
}

#[cfg(test)]
mod tests;
