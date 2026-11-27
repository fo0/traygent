//! `state.json` in the app data dir: settings, recent sessions and the connected GitHub
//! account's metadata (never its token) in one versioned file.
//!
//! Writes are atomic (temp file + rename). A file from a newer Traygent is read but never
//! overwritten; a corrupt file is renamed to `state.json.corrupt-<timestamp>` and replaced
//! by defaults. Unknown fields are ignored; missing, unreadable or invalid ones take their
//! default ([`settings::sanitize`]).
use crate::models::{
    AgentId, AgentProfile, AppSettings, GithubAccount, LaunchOptions, RecentSession,
    DEFAULT_PROFILE,
};
use crate::{app_log::Level, github, recents, settings};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub const FILE: &str = "state.json";
pub const VERSION: u64 = 1;
// The read-only notices double as the error of every refused save.
pub const NEWER_VERSION: &str = "Einstellungen und Sessions werden nicht gespeichert: state.json stammt aus einer neueren Traygent-Version. Bitte Traygent aktualisieren.";
pub const UNREADABLE: &str = "Einstellungen und Sessions werden nicht gespeichert: state.json konnte nicht gelesen werden und bleibt unverändert.";
pub const NO_APP_DATA: &str =
    "Einstellungen und Sessions werden nicht gespeichert: kein App-Datenordner verfügbar.";
const SAVE_FAILED: &str = "Einstellungen konnten nicht gespeichert werden.";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Persisted {
    pub settings: AppSettings,
    pub recents: Vec<RecentSession>,
    /// Login, id and source of the connected account; its token lives in the OS vault.
    pub github: Option<GithubAccount>,
}

#[derive(Debug, Default)]
pub struct Loaded {
    pub state: Persisted,
    /// German notice when the file must not be written (newer version, unreadable).
    pub read_only: Option<String>,
    /// Where a corrupt file was moved.
    pub backup: Option<PathBuf>,
}

pub fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

/// Read-only lookup for the credential helper. An unreadable or incompatible state
/// refuses forwarding; a helper never repairs, moves or writes the settings file.
pub(crate) fn docker_github_account_id(dir: &Path) -> Option<u64> {
    let bytes = std::fs::read(path(dir)).ok()?;
    let state: Value = serde_json::from_slice(&bytes).ok()?;
    if state.get("version")?.as_u64()? != VERSION
        || !state.pointer("/settings/docker/ghToken")?.as_bool()?
    {
        return None;
    }
    parse_account(state.get("github")?).map(|account| account.id)
}

pub fn load(dir: &Path) -> Loaded {
    let file = path(dir);
    let text = match std::fs::read(&file) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Loaded::default(),
        Err(_) => {
            return Loaded {
                read_only: Some(UNREADABLE.into()),
                ..Loaded::default()
            }
        }
    };
    let parsed = serde_json::from_slice::<Value>(&text).ok();
    let object = parsed.as_ref().and_then(Value::as_object);
    let version = object.and_then(|object| object.get("version")?.as_u64());
    match (object, version) {
        (Some(object), Some(version)) if version >= 1 => Loaded {
            state: parse(object),
            read_only: (version > VERSION).then(|| NEWER_VERSION.into()),
            backup: None,
        },
        _ => back_up(&file),
    }
}

/// Moves a corrupt file aside; if that fails, defaults are used but nothing is written.
fn back_up(file: &Path) -> Loaded {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let mut backup = file.with_file_name(format!("{FILE}.corrupt-{stamp}"));
    let mut counter = 1;
    while backup.exists() {
        counter += 1;
        backup = file.with_file_name(format!("{FILE}.corrupt-{stamp}-{counter}"));
    }
    match std::fs::rename(file, &backup) {
        Ok(()) => Loaded {
            backup: Some(backup),
            ..Loaded::default()
        },
        Err(_) => Loaded {
            read_only: Some(UNREADABLE.into()),
            ..Loaded::default()
        },
    }
}

fn parse(object: &Map<String, Value>) -> Persisted {
    let settings = object
        .get("settings")
        .and_then(Value::as_object)
        .map(parse_settings)
        .unwrap_or_default();
    let recents = object
        .get("recents")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|item| serde_json::from_value::<RecentSession>(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    Persisted {
        settings: settings::sanitize(settings),
        recents: recents::tidy(recents),
        github: object.get("github").and_then(parse_account),
    }
}

/// An account with an unusable login or id is dropped; the user connects again.
fn parse_account(value: &Value) -> Option<GithubAccount> {
    serde_json::from_value::<GithubAccount>(value.clone())
        .ok()
        .filter(|account| account.id != 0 && github::valid_login(&account.login))
}

/// Field by field, so one unknown value (e.g. a terminal a newer version added) only resets
/// that field instead of every setting.
fn parse_settings(object: &Map<String, Value>) -> AppSettings {
    let defaults = AppSettings::default();
    let agents = object.get("agents").and_then(Value::as_object);
    let quota_bars = object.get("quotaBars").and_then(Value::as_object);
    let quota_reads = object.get("quotaReads").and_then(Value::as_object);
    let quota_profiles = object.get("quotaProfiles").and_then(Value::as_object);
    let docker = object.get("docker").and_then(Value::as_object);
    let images = docker.and_then(|docker| docker.get("images")?.as_object().cloned());
    let profiles = object.get("profiles").and_then(Value::as_object);
    let agent_profile = object.get("agentProfile").and_then(Value::as_object);
    let share = object.get("profileShare").and_then(Value::as_object);
    let mut settings = AppSettings {
        default_agent: field(object, "defaultAgent", defaults.default_agent),
        default_env: field(object, "defaultEnv", defaults.default_env),
        environments: known_items(object.get("environments"))
            .unwrap_or_else(|| defaults.environments.clone()),
        terminal: field(object, "terminal", defaults.terminal),
        shared_window: field(object, "sharedWindow", defaults.shared_window),
        agent_updates: field(object, "agentUpdates", defaults.agent_updates),
        quota_refresh_minutes: field(
            object,
            "quotaRefreshMinutes",
            defaults.quota_refresh_minutes,
        ),
        log_level: log_level(object),
        ..defaults.clone()
    };
    for agent in AgentId::ALL {
        if let Some(agents) = agents {
            *settings.agents.get_mut(agent) = field(agents, agent.id(), LaunchOptions::default());
        }
        if let Some(list) = known_items(quota_bars.and_then(|bars| bars.get(agent.id()))) {
            *settings.quota_bars.get_mut(agent) = list;
        }
        // Absent (a file from before the setting) or unreadable: the agent's default.
        if let Some(reads) = quota_reads {
            let fallback = *defaults.quota_reads.get(agent);
            *settings.quota_reads.get_mut(agent) = field(reads, agent.id(), fallback);
        }
        // Absent (a file from before the setting): the start profile only.
        if let Some(list) = quota_profiles.and_then(|shown| shown.get(agent.id())) {
            *settings.quota_profiles.get_mut(agent) = names(Some(list));
        }
        if let Some(images) = &images {
            let fallback = defaults.docker.images.get(agent).clone();
            *settings.docker.images.get_mut(agent) = field(images, agent.id(), fallback);
        }
        // A file from before the profiles has neither: no extra profile, `default` chosen.
        if let Some(list) = profiles.and_then(|profiles| profiles.get(agent.id())?.as_array()) {
            *settings.profiles.get_mut(agent) = list.iter().filter_map(parse_profile).collect();
        }
        // Absent (a file from before the setting): nothing extra is shared.
        if let Some(share) = share.and_then(|share| share.get(agent.id())?.as_object()) {
            let shared = settings.profile_share.get_mut(agent);
            shared.dirs = names(share.get("dirs"));
            shared.files = names(share.get("files"));
        }
        if let Some(chosen) = agent_profile {
            *settings.agent_profile.get_mut(agent) =
                field(chosen, agent.id(), DEFAULT_PROFILE.to_string());
        }
    }
    if let Some(docker) = docker {
        settings.docker.gh_token = field(docker, "ghToken", false);
        settings.docker.ports = field(docker, "ports", Vec::new());
        settings.docker.socket = field(docker, "socket", false);
        // Absent (a file from before the setting) or unreadable: on, like a fresh install.
        settings.docker.manage_node = field(docker, "manageNode", defaults.docker.manage_node);
    }
    if let Some(github) = object.get("github").and_then(Value::as_object) {
        settings.github.clone_root = field(github, "cloneRoot", String::new());
    }
    settings
}

/// `logLevel` (#109); a file from before it keeps a stored „Debug“ (`debug: true`, #41) as
/// `debug`. An unknown value (a newer build's level) reads as `info`.
fn log_level(object: &Map<String, Value>) -> Level {
    match object.get("logLevel") {
        Some(value) => serde_json::from_value(value.clone()).unwrap_or_default(),
        None if object.get("debug") == Some(&Value::Bool(true)) => Level::Debug,
        None => Level::Info,
    }
}

/// The strings of a list; anything else in it is dropped. [`settings::sanitize`] validates.
fn names(value: Option<&Value>) -> Vec<String> {
    let list = value.and_then(Value::as_array).into_iter().flatten();
    list.filter_map(|name| name.as_str().map(str::to_string))
        .collect()
}

/// One malformed entry is dropped, the other profiles kept; [`settings::sanitize`] validates.
fn parse_profile(value: &Value) -> Option<AgentProfile> {
    serde_json::from_value(value.clone()).ok()
}

/// Drop unknown list entries individually. Missing or non-list fields use their default.
fn known_items<T: DeserializeOwned>(value: Option<&Value>) -> Option<Vec<T>> {
    let list = value?.as_array()?;
    let known = list
        .iter()
        .filter_map(|item| serde_json::from_value(item.clone()).ok());
    Some(known.collect())
}

fn field<T: DeserializeOwned>(object: &Map<String, Value>, key: &str, fallback: T) -> T {
    object
        .get(key)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or(fallback)
}

#[derive(serde::Serialize)]
struct File<'a> {
    version: u64,
    settings: &'a AppSettings,
    recents: &'a [RecentSession],
    #[serde(skip_serializing_if = "Option::is_none")]
    github: Option<&'a GithubAccount>,
}

/// Writes the whole state atomically: a sibling temp file, flushed, then renamed over.
pub fn save(dir: &Path, state: &Persisted) -> Result<(), String> {
    let file = File {
        version: VERSION,
        settings: &state.settings,
        recents: &state.recents,
        github: state.github.as_ref(),
    };
    let mut text = serde_json::to_vec_pretty(&file).map_err(|_| SAVE_FAILED)?;
    text.push(b'\n');
    std::fs::create_dir_all(dir).map_err(|_| SAVE_FAILED)?;
    let temp = dir.join(format!("{FILE}.tmp-{}", std::process::id()));
    let written = write_synced(&temp, &text).and_then(|()| std::fs::rename(&temp, path(dir)));
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
        return Err(SAVE_FAILED.into());
    }
    Ok(())
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests;
