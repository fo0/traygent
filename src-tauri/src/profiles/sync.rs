//! The profile sync: after it every profile of an agent differs from the default home only
//! in its sign-in and its own identity keys.
//!
//! Std file system only, over paths [`Homes`] built. Nothing is ever deleted except an empty
//! directory that makes room for a link; what a rule replaces goes to the profile's backup
//! folder first. One sync runs at a time.
use super::files::{problem, sync_file, write_atomic, Backup, Backups, Change, ACCESS_FAILED};
use super::history::sync_history;
use super::json::sync_state;
use super::links::{self, Link, Linker, Pending};
use super::status::{self, Facts, Key};
use super::{recipe, Homes, Recipe};
use crate::models::{AgentId, AgentProfile, AppSettings, DEFAULT_PROFILE};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

pub const MARKER: &str = ".traygent-sync.json";
const MARKER_VERSION: u64 = 1;
pub const NO_HOME: &str = "Benutzerordner nicht verfügbar";
pub const PROFILE_DIR_FAILED: &str = "Profilordner konnte nicht angelegt werden";
const DEFAULT_DIR_FAILED: &str = "Ordner des Standard-Profils konnte nicht angelegt werden";
const IS_DEFAULT_HOME: &str = "Profilordner ist der Ordner des Standard-Profils";
const DIR_UNCHECKED: &str = "Profilordner konnte nicht geprüft werden";

/// An existing, non-empty directory without the CLI's state file, its sign-in or the marker
/// is someone else's (`.claude-mem`, `.claude-backup`): nothing in it is touched.
pub fn foreign_dir(agent: AgentId) -> String {
    format!("Profilordner gehört nicht zu {}", agent.label())
}

static RUNNING: Mutex<()> = Mutex::new(());

/// `<profile dir>\.traygent-sync.json`: `done` names the rules whose first sync succeeded —
/// until then the default home wins for that rule, afterwards the newest. `syncedAt` is the
/// last sync without an error.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<String>,
    #[serde(default)]
    pub done: Vec<String>,
}

/// `None` without a marker file; an unreadable one still counts as a marker.
pub fn read_marker(dir: &Path) -> Option<Marker> {
    let bytes = std::fs::read(dir.join(MARKER)).ok()?;
    Some(serde_json::from_slice(&bytes).unwrap_or_default())
}

/// How the sync of one profile went.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileReport {
    pub id: String,
    pub label: String,
    /// German, one entry per thing that could not be done; empty = in sync.
    pub errors: Vec<String>,
    /// The profile's directory cannot be used: a session cannot start in it.
    pub dir_failed: bool,
    /// … because it exists and is not this agent's ([`foreign_dir`]).
    pub foreign: bool,
    /// The run created a link or wrote a file of the pair (the marker does not count).
    pub changed: bool,
}

/// What to sync or read, taken from the settings under the service lock; the file system
/// work then runs without it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncPlan {
    pub homes: Option<Homes>,
    /// Every agent with a recipe and its extra profiles (possibly none).
    pub agents: Vec<(AgentId, Vec<AgentProfile>)>,
}

/// The result of a [`SyncPlan`]: the reports of the agents that were synced and every
/// profile's facts, read afterwards.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncOutcome {
    /// `None`: the plan only read the facts.
    pub reports: Option<Vec<(AgentId, Vec<ProfileReport>)>>,
    pub facts: HashMap<Key, Facts>,
}

impl SyncPlan {
    pub fn of(homes: Option<&Homes>, settings: &AppSettings) -> Self {
        let agents = AgentId::ALL
            .into_iter()
            .filter(|agent| recipe(*agent).is_some())
            .map(|agent| (agent, settings.profiles.get(agent).clone()))
            .collect();
        Self {
            homes: homes.cloned(),
            agents,
        }
    }

    /// The same plan for one agent.
    pub fn only(mut self, agent: AgentId) -> Self {
        self.agents.retain(|(listed, _)| *listed == agent);
        self
    }

    /// Reads every profile's facts without syncing.
    pub fn read(&self) -> SyncOutcome {
        SyncOutcome {
            reports: None,
            facts: status::read_all(self.homes.as_ref(), &self.agents),
        }
    }

    /// Syncs every agent that has an extra profile, then reads the facts. An agent without
    /// one is not touched: nothing is created for it.
    pub fn run(&self, linker: &dyn Linker, now: DateTime<Utc>) -> SyncOutcome {
        let reports = self
            .agents
            .iter()
            .map(|(agent, profiles)| {
                let reports = match &self.homes {
                    Some(homes) => sync_agent(homes, *agent, profiles, linker, now),
                    None => failed(profiles, NO_HOME, false),
                };
                (*agent, reports)
            })
            .collect();
        SyncOutcome {
            reports: Some(reports),
            ..self.read()
        }
    }
}

fn failed(profiles: &[AgentProfile], error: &str, dir_failed: bool) -> Vec<ProfileReport> {
    profiles
        .iter()
        .map(|profile| ProfileReport {
            id: profile.id.clone(),
            label: profile.label.clone(),
            errors: vec![error.to_string()],
            dir_failed,
            ..ProfileReport::default()
        })
        .collect()
}

/// One profile and the default home it is synced with.
struct Pair {
    report: ProfileReport,
    dir: PathBuf,
    state: PathBuf,
    /// As read; `None` before the first sync.
    marker: Option<Marker>,
    /// The rules whose first sync succeeded, from the marker plus this run.
    done: Vec<String>,
    backup: Backup,
    /// False when the directory is unusable; no rule runs then.
    usable: bool,
}

/// An existing directory that holds something but none of the agent's own files.
fn is_foreign(dir: &Path, recipe: &Recipe) -> std::io::Result<bool> {
    if std::fs::read_dir(dir)?.next().is_none() {
        return Ok(false);
    }
    let own = [recipe.state_file, recipe.credentials, MARKER];
    let known = |name: &&str| std::fs::symlink_metadata(dir.join(name)).is_ok();
    Ok(!own.iter().any(known))
}

impl Pair {
    fn open(
        homes: &Homes,
        agent: AgentId,
        recipe: &Recipe,
        profile: &AgentProfile,
        default_dir: &Path,
        now: DateTime<Utc>,
    ) -> Self {
        let dir = homes.dir(agent, &profile.id).map(PathBuf::from);
        let state = homes.state_file(agent, &profile.id).map(PathBuf::from);
        let mut report = ProfileReport {
            id: profile.id.clone(),
            label: profile.label.clone(),
            ..ProfileReport::default()
        };
        // Only a validated id of an extra profile yields a directory.
        let paths = dir.zip(state).filter(|_| profile.id != DEFAULT_PROFILE);
        let (dir, state) = paths.clone().unwrap_or_default();
        let refusal = match &paths {
            None => Some((PROFILE_DIR_FAILED.to_string(), true)),
            Some(_) if std::fs::symlink_metadata(&dir).is_ok() => {
                // E.g. a profile directory that is itself a link to the default home: there
                // is nothing to sync, and adopting its directories would move the default
                // home's. A check without an answer refuses as well.
                match (
                    links::same_place(&dir, default_dir),
                    is_foreign(&dir, recipe),
                ) {
                    (Ok(true), _) => Some((IS_DEFAULT_HOME.to_string(), false)),
                    (Ok(false), Ok(true)) => {
                        report.foreign = true;
                        Some((foreign_dir(agent), true))
                    }
                    (Ok(false), Ok(false)) => None,
                    // A file where the directory belongs.
                    _ if !dir.is_dir() => Some((PROFILE_DIR_FAILED.to_string(), true)),
                    _ => Some((DIR_UNCHECKED.to_string(), false)),
                }
            }
            Some(_) => std::fs::create_dir_all(&dir)
                .is_err()
                .then(|| (PROFILE_DIR_FAILED.to_string(), true)),
        };
        if let Some((error, dir_failed)) = &refusal {
            report.errors.push(error.clone());
            report.dir_failed = *dir_failed;
        }
        let usable = refusal.is_none();
        let marker = usable.then(|| read_marker(&dir)).flatten();
        Self {
            report,
            done: marker.clone().unwrap_or_default().done,
            marker,
            backup: Backup::new(&dir, now),
            dir,
            state,
            usable,
        }
    }

    /// Notes a rule's result; a rule that succeeded has had its first sync.
    fn note(&mut self, rule: Option<&str>, result: Result<Change, String>) -> Change {
        match result {
            Ok(change) => {
                self.report.changed |= change.any();
                if let Some(rule) = rule.filter(|rule| !self.done.iter().any(|done| done == rule)) {
                    self.done.push(rule.to_string());
                }
                change
            }
            Err(error) => {
                if !self.report.errors.contains(&error) {
                    self.report.errors.push(error);
                }
                Change::default()
            }
        }
    }

    fn first(&self, rule: &str) -> bool {
        !self.done.iter().any(|done| done == rule)
    }

    /// The file rules of the pair; says whether the default home's side changed. A rule is
    /// a first sync (the default home wins) until it succeeded once — also in the second
    /// pass of a run, and in the next run after one that failed.
    fn files(
        &mut self,
        recipe: &Recipe,
        default_dir: &Path,
        default_state: &Path,
        default_backup: &mut Backup,
    ) -> bool {
        let mut change = Change::default();
        for name in recipe.synced_files {
            let (ours, theirs) = (default_dir.join(name), self.dir.join(name));
            let first = self.first(name);
            let mut backups = Backups {
                default: default_backup,
                profile: &mut self.backup,
            };
            let result = sync_file(name, &ours, &theirs, first, &mut backups);
            change.merge(self.note(Some(name), result));
        }
        if let Some(name) = recipe.history {
            let result = sync_history(name, &default_dir.join(name), &self.dir.join(name));
            change.merge(self.note(None, result));
        }
        let name = recipe.state_file;
        let first = self.first(name);
        let mut backups = Backups {
            default: default_backup,
            profile: &mut self.backup,
        };
        let keys = recipe.shared_keys;
        let result = sync_state(name, default_state, &self.state, first, keys, &mut backups);
        change.merge(self.note(Some(name), result));
        change.default
    }

    /// Writes the marker when it changed: the rules done so far, and a new `syncedAt` only
    /// after a sync without an error.
    fn mark(&mut self, now: DateTime<Utc>) {
        let previous = self.marker.clone().unwrap_or_default();
        let marker = Marker {
            version: MARKER_VERSION,
            synced_at: if self.report.errors.is_empty() {
                Some(crate::recents::timestamp(now))
            } else {
                previous.synced_at
            },
            done: self.done.clone(),
        };
        if self.marker.as_ref() == Some(&marker) {
            return;
        }
        let written = serde_json::to_vec_pretty(&marker)
            .map_err(drop)
            .and_then(|bytes| write_atomic(&self.dir.join(MARKER), &bytes).map_err(drop));
        if written.is_err() {
            self.report.errors.push(problem(MARKER, ACCESS_FAILED));
        }
    }
}

/// Syncs every extra profile of `agent` with its default home: the shared directories become
/// links (all missing ones created in one linker call), then the synced files, the history
/// and the shared keys of the state file. With no extra profile nothing is touched.
pub fn sync_agent(
    homes: &Homes,
    agent: AgentId,
    profiles: &[AgentProfile],
    linker: &dyn Linker,
    now: DateTime<Utc>,
) -> Vec<ProfileReport> {
    let Some(recipe) = recipe(agent).filter(|_| !profiles.is_empty()) else {
        return Vec::new();
    };
    let default = homes
        .dir(agent, DEFAULT_PROFILE)
        .zip(homes.state_file(agent, DEFAULT_PROFILE));
    let Some((default_dir, default_state)) = default else {
        return failed(profiles, NO_HOME, false);
    };
    let (default_dir, default_state) = (PathBuf::from(default_dir), PathBuf::from(default_state));
    let _running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    if std::fs::create_dir_all(&default_dir).is_err() {
        return failed(profiles, DEFAULT_DIR_FAILED, false);
    }
    let mut default_backup = Backup::new(&default_dir, now);
    let mut pairs: Vec<Pair> = profiles
        .iter()
        .map(|profile| Pair::open(homes, agent, recipe, profile, &default_dir, now))
        .collect();

    let mut wanted: Vec<(usize, &str, Pending)> = Vec::new();
    for (index, pair) in pairs.iter_mut().enumerate().filter(|(_, pair)| pair.usable) {
        for name in recipe.shared_dirs {
            let (target, link) = (default_dir.join(name), pair.dir.join(name));
            match links::prepare(name, &target, &link, &mut pair.backup) {
                Ok(Some(pending)) => wanted.push((index, name, pending)),
                Ok(None) => {}
                Err(error) => pair.report.errors.push(error),
            }
        }
    }
    if !wanted.is_empty() {
        let list: Vec<Link> = wanted
            .iter()
            .map(|(_, _, pending)| pending.link.clone())
            .collect();
        linker.link(&list);
        for (index, name, pending) in &wanted {
            let pair = &mut pairs[*index];
            match links::verify(name, &pending.link) {
                Ok(()) => pair.report.changed = true,
                Err(error) => {
                    pending.undo(&mut pair.backup);
                    pair.report.errors.push(error);
                }
            }
        }
    }

    let mut default_changed = false;
    for pair in pairs.iter_mut().filter(|pair| pair.usable) {
        default_changed |= pair.files(recipe, &default_dir, &default_state, &mut default_backup);
    }
    // What one profile handed to the default home reaches the others in the same run.
    if default_changed && pairs.len() > 1 {
        for pair in pairs.iter_mut().filter(|pair| pair.usable) {
            pair.files(recipe, &default_dir, &default_state, &mut default_backup);
        }
    }
    for pair in pairs.iter_mut().filter(|pair| pair.usable) {
        pair.mark(now);
    }
    pairs.into_iter().map(|pair| pair.report).collect()
}
