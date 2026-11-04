//! The native service behind the IPC commands: owns settings, recent sessions and the last
//! detection, persists through [`store`] and builds every [`Snapshot`] the panel receives.
//! Every state change bumps `revision`, so the panel can drop a snapshot that arrives late.
use crate::detect::Detection;
use crate::launch;
use crate::models::{
    AgentId, AgentStatus, AppSettings, EnvId, EnvStatus, GithubState, LaunchOptions, LaunchRequest,
    ProbeState, RecentSession, Snapshot,
};
use crate::store::{self, Persisted};
use crate::{catalog, ports, recents, settings};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

mod github;
mod launch_prep;
pub use github::Listing;
pub use launch_prep::Prepared;

pub const RECENT_GONE: &str = "Diese Session steht nicht mehr in „Zuletzt“.";
pub const SAVE_FAILED: &str = "Einstellungen und Sessions werden nicht gespeichert: state.json konnte nicht geschrieben werden.";

pub struct Service {
    dir: PathBuf,
    state: Persisted,
    /// German notice when `state.json` must not be written.
    read_only: Option<String>,
    /// German notice after a failed write behind a start; the next successful write clears it.
    save_failed: Option<String>,
    detection: Option<Detection>,
    detecting: bool,
    /// The running detection probes Docker images that changed since; it runs again.
    stale: bool,
    revision: u64,
    /// The account and listing the snapshot shows; `defaultCloneRoot` is the host's.
    github: GithubState,
    /// Increases whenever the account changes; a listing begun before is dropped.
    github_generation: u64,
}

/// A saved settings change; `start_detection` asks the caller to start one (the Docker
/// images changed and none ran; a running one was marked stale and reruns by itself).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub snapshot: Snapshot,
    pub start_detection: bool,
}

/// How a detection run ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DetectionEnd {
    Finished(Box<Snapshot>),
    /// The settings it probed changed: its result is dropped, `detecting` stays on and the
    /// caller probes again with the current images.
    Rerun,
}

impl Service {
    /// Loads `state.json` from `dir` (the app data dir; tests pass a temp dir).
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let loaded = store::load(&dir);
        if let Some(backup) = &loaded.backup {
            eprintln!("Traygent: unreadable state moved to {}", backup.display());
        }
        Self::with(dir, loaded.state, loaded.read_only)
    }

    /// Defaults that are never written, e.g. when no app data dir exists.
    pub fn read_only(notice: &str) -> Self {
        Self::with(PathBuf::new(), Persisted::default(), Some(notice.into()))
    }

    fn with(dir: PathBuf, state: Persisted, read_only: Option<String>) -> Self {
        Self {
            dir,
            read_only,
            save_failed: None,
            detection: None,
            detecting: false,
            stale: false,
            revision: 0,
            github: GithubState {
                account: state.github.clone(),
                ..GithubState::default()
            },
            github_generation: 0,
            state,
        }
    }

    pub fn github(&self) -> &GithubState {
        &self.github
    }

    /// The snapshot's `defaultCloneRoot`, resolved from the host once at startup
    /// ([`crate::repo::default_clone_root`]); a launch resolves it from its own host.
    pub fn set_default_clone_root(&mut self, root: String) {
        self.github.default_clone_root = root;
        self.touch();
    }

    pub fn settings(&self) -> &AppSettings {
        &self.state.settings
    }

    /// The persistent warning the panel shows: read-only state, or the last write failed.
    pub fn notice(&self) -> Option<&str> {
        self.read_only.as_deref().or(self.save_failed.as_deref())
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut snapshot = build_snapshot(
            &self.state.settings,
            &self.state.recents,
            self.detection.as_ref(),
            self.detecting,
        );
        snapshot.revision = self.revision;
        snapshot.github = self.github.clone();
        snapshot.notice = self.notice().map(Into::into);
        snapshot
    }

    fn touch(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }

    /// Marks a detection as running; `false` when one already runs (no second probe storm).
    pub fn begin_detection(&mut self) -> bool {
        if std::mem::replace(&mut self.detecting, true) {
            return false;
        }
        self.touch();
        true
    }

    /// Ends the running detection; `None` (a failed run) keeps the previous results.
    pub fn finish_detection(&mut self, detection: Option<Detection>) -> DetectionEnd {
        if std::mem::take(&mut self.stale) {
            return DetectionEnd::Rerun;
        }
        self.detecting = false;
        if detection.is_some() {
            self.detection = detection;
        }
        self.touch();
        DetectionEnd::Finished(Box::new(self.snapshot()))
    }

    /// Clears a detection that never ran (its thread could not start).
    pub fn abort_detection(&mut self) {
        self.detecting = false;
        self.stale = false;
        self.touch();
    }

    fn persist(&mut self, next: &Persisted) -> Result<(), String> {
        if let Some(notice) = &self.read_only {
            return Err(notice.clone());
        }
        store::save(&self.dir, next)?;
        self.save_failed = None;
        Ok(())
    }

    /// Persists a change that already happened outside (a spawned terminal, a vault entry):
    /// a failed save only loses it on disk and becomes the snapshot's notice.
    fn persist_or_notice(&mut self, next: &Persisted) {
        if let Err(error) = self.persist(next) {
            eprintln!("Traygent: {error}");
            if self.read_only.is_none() {
                self.save_failed = Some(SAVE_FAILED.into());
            }
        }
    }

    pub fn save_settings(&mut self, settings: AppSettings) -> Result<Saved, String> {
        let settings = settings::normalize(settings);
        settings::validate(&settings)?;
        let next = Persisted {
            settings,
            ..self.state.clone()
        };
        self.persist(&next)?;
        let images_changed = next.settings.docker.images != self.state.settings.docker.images;
        self.state = next;
        self.touch();
        // The Docker statuses depend on the images; a detection with the old ones is stale.
        let start_detection = images_changed
            && if self.detecting {
                self.stale = true;
                false
            } else {
                self.begin_detection()
            };
        Ok(Saved {
            snapshot: self.snapshot(),
            start_detection,
        })
    }

    fn commit_recents(&mut self, recents: Vec<RecentSession>) -> Result<Snapshot, String> {
        let next = Persisted {
            recents,
            ..self.state.clone()
        };
        self.persist(&next)?;
        self.state = next;
        self.touch();
        Ok(self.snapshot())
    }

    /// Removing an id that is not listed changes nothing.
    pub fn remove_recent(&mut self, id: &str) -> Result<Snapshot, String> {
        let mut list = self.state.recents.clone();
        if recents::remove(&mut list, id).is_none() {
            return Ok(self.snapshot());
        }
        self.commit_recents(list)
    }

    pub fn restore_recent(&mut self, recent: RecentSession) -> Result<Snapshot, String> {
        if recent.env == EnvId::Wsl {
            return Err(launch::WSL_SOON.into());
        }
        let folder = launch::normalize_folder(&recent.folder)?;
        let options = settings::normalize_options(recent.options.clone());
        settings::validate_options(recent.agent, &options)?;
        let ports = ports::normalize_ports(&recent.ports)?;
        if !ports.is_empty() && recent.env != EnvId::Docker {
            return Err(ports::DOCKER_ONLY.into());
        }
        let repo = recent
            .repo
            .as_ref()
            .map(crate::repo::validate)
            .transpose()?;
        let mut list = self.state.recents.clone();
        let recent = RecentSession {
            folder,
            repo,
            options,
            ports,
            ..recent
        };
        recents::restore(&mut list, recent)?;
        self.commit_recents(list)
    }

    /// „Bearbeiten …“ of a „Zuletzt“ row: replaces the options and ports the next restart
    /// sends, validated like a start; the entry keeps its place, start time and count.
    pub fn update_recent(
        &mut self,
        id: &str,
        options: LaunchOptions,
        ports: Vec<String>,
    ) -> Result<Snapshot, String> {
        let mut list = self.state.recents.clone();
        let recent = list
            .iter_mut()
            .find(|recent| recent.id == id)
            .ok_or(RECENT_GONE)?;
        let options = settings::normalize_options(options);
        settings::validate_options(recent.agent, &options)?;
        let ports = ports::normalize_ports(&ports)?;
        if !ports.is_empty() && recent.env != EnvId::Docker {
            return Err(ports::DOCKER_ONLY.into());
        }
        recent.options = options;
        recent.ports = ports;
        self.commit_recents(list)
    }

    /// Records a start after its terminal spawned. A failed save only loses the history
    /// entry on disk; the session already runs, so it becomes the snapshot's notice.
    pub fn record_launch(&mut self, request: &LaunchRequest, now: DateTime<Utc>) -> RecentSession {
        let mut next = self.state.clone();
        let recent = recents::upsert(&mut next.recents, request, now);
        self.persist_or_notice(&next);
        self.state = next;
        self.touch();
        recent
    }
}

/// Statuses before any detection: probes `unknown`, `local` ready, `wsl` soon.
pub fn initial_statuses() -> (Vec<AgentStatus>, Vec<EnvStatus>) {
    let agents = AgentId::ALL
        .into_iter()
        .flat_map(|agent| {
            [EnvId::Local, EnvId::Docker]
                .map(|env| crate::detect::status(agent, env, ProbeState::Unknown))
        })
        .collect();
    let env = |env, state, detail: Option<&str>| EnvStatus {
        env,
        state,
        version: None,
        detail: detail.map(Into::into),
    };
    let environments = vec![
        env(EnvId::Local, ProbeState::Ready, None),
        env(EnvId::Docker, ProbeState::Unknown, None),
        env(
            EnvId::Wsl,
            ProbeState::Unavailable,
            Some(crate::detect::SOON),
        ),
    ];
    (agents, environments)
}

pub fn build_snapshot(
    settings: &AppSettings,
    recents: &[RecentSession],
    detection: Option<&Detection>,
    detecting: bool,
) -> Snapshot {
    let (agents, environments) = match detection {
        Some(found) => (found.agents.clone(), found.environments.clone()),
        None => initial_statuses(),
    };
    Snapshot {
        demo: false,
        settings: settings.clone(),
        catalog: catalog::catalog(),
        port_presets: ports::presets(),
        agents,
        environments,
        recents: recents.to_vec(),
        github: GithubState::default(),
        detecting,
        revision: 0,
        notice: None,
        checked_at: detection.map(|found| found.checked_at.clone()),
    }
}

impl Snapshot {
    /// The native shell's snapshot before any detection ran: defaults, no recents.
    pub fn native() -> Self {
        build_snapshot(&AppSettings::default(), &[], None, false)
    }
}

/// Six hex digits for a container name; uniqueness, not secrecy.
pub fn nonce() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    hasher.write_u128(now);
    format!("{:06x}", hasher.finish() & 0x00ff_ffff)
}

#[cfg(test)]
mod tests;
