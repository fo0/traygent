//! Selects what a quota read uses for the requested session account: the credential file
//! (Codex, OpenCode Go) or, for Claude Code, the profile directory its CLI reads with (#110).
use crate::detect::{host::is_search_dir, Fs, Host, RealFs};
use crate::launch::{docker::AgentHome, is_network_path, normalize_folder, CLAUDE_LOGIN_OVERRIDES};
use crate::models::{AgentId, AppSettings, EnvId, DEFAULT_PROFILE};
use crate::{profiles, service::PROFILE_MISSING};
use std::path::PathBuf;

const NO_HOME: &str = "Der Anmeldeordner ist nicht verfügbar.";
const DOCKER_OPENCODE: &str =
    "OpenCode verwendet in Docker eine eigene Anmeldung im Docker-Volume.";
const WSL: &str = "Für WSL ist keine Kontingentabfrage verfügbar.";
const ENV_LOGIN: &str =
    "Für die Anmeldung aus dieser Umgebung ist keine Kontingentabfrage verfügbar.";

/// What a quota read of the account uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuotaSource {
    /// The sign-in file or database the provider request reads its key from.
    File(PathBuf),
    /// Claude Code's own CLI (`/usage`, ADR-0015) in this profile directory, checked like a
    /// start's; `None` = the default profile, which sets no `CLAUDE_CONFIG_DIR`.
    ClaudeCli { profile_dir: Option<String> },
}

/// A saved profile ID is the only caller-supplied component that can enter a path.
/// Run this on the quota worker: a named profile's directory is checked for links.
pub fn quota_source(
    agent: AgentId,
    profile: Option<&str>,
    env: EnvId,
    settings: &AppSettings,
    host: &Host,
) -> Result<QuotaSource, &'static str> {
    choose_source(agent, profile, env, settings, host, |name| {
        std::env::var_os(name).is_some_and(|value| {
            if name.starts_with("CLAUDE_CODE_USE_") {
                matches!(value.to_str(), Some("1")) || value.eq_ignore_ascii_case("true")
            } else {
                !value.is_empty()
            }
        })
    })
}

fn choose_source(
    agent: AgentId,
    profile: Option<&str>,
    env: EnvId,
    settings: &AppSettings,
    host: &Host,
    configured: impl Fn(&str) -> bool,
) -> Result<QuotaSource, &'static str> {
    if env == EnvId::Wsl {
        return Err(WSL);
    }
    if agent == AgentId::Opencode && env == EnvId::Docker {
        return Err(DOCKER_OPENCODE);
    }
    let id = profile.unwrap_or(DEFAULT_PROFILE);
    if id != DEFAULT_PROFILE
        && !(profiles::supported(agent, env)
            && profiles::is_valid_id(id)
            && settings.profiles.get(agent).iter().any(|p| p.id == id))
    {
        return Err(PROFILE_MISSING);
    }
    // Local sessions inherit credentials. Managed Claude containers clear them;
    // unmanaged ones forward only these two variables (launch/docker.rs). With such a
    // sign-in the CLI would report no subscription either.
    let overrides: &[&str] = match (agent, env) {
        (AgentId::Claude, EnvId::Local) => &CLAUDE_LOGIN_OVERRIDES,
        (AgentId::Claude, EnvId::Docker) if settings.profiles.claude.is_empty() => {
            &["ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN"]
        }
        // The interactive Codex CLI checks CODEX_ACCESS_TOKEN ahead of its stored login.
        // CODEX_API_KEY is for exec, and OPENAI_API_KEY alone does not select API-key login.
        (AgentId::Codex, EnvId::Local) => &["CODEX_ACCESS_TOKEN"],
        _ => &[],
    };
    if overrides.iter().any(|name| configured(name)) {
        return Err(ENV_LOGIN);
    }

    if agent == AgentId::Opencode {
        let database = local_path(opencode_database(host).ok_or(NO_HOME)?, host.windows)?;
        return Ok(QuotaSource::File(PathBuf::from(database)));
    }
    if agent == AgentId::Claude && id == DEFAULT_PROFILE {
        // The CLI finds its own default home, as a start does.
        return Ok(QuotaSource::ClaudeCli { profile_dir: None });
    }
    let home = AgentHome::from_host(agent, host, settings, Some(id))
        .map_err(|_| NO_HOME)?
        .ok_or(NO_HOME)?;
    let directory = local_path(home.directory, host.windows)?;
    if id != DEFAULT_PROFILE && RealFs.is_link(&directory) {
        return Err("Der Profilordner darf keine Verknüpfung sein.");
    }
    if agent == AgentId::Claude {
        return Ok(QuotaSource::ClaudeCli {
            profile_dir: Some(directory),
        });
    }
    let file = profiles::recipe(agent).map_or("auth.json", |recipe| recipe.credentials);
    Ok(QuotaSource::File(PathBuf::from(
        host.join(&directory, &[file]),
    )))
}

fn local_path(path: String, windows: bool) -> Result<String, &'static str> {
    if !is_search_dir(&path, windows) {
        return Err(NO_HOME);
    }
    if !windows {
        return Ok(path);
    }
    let local = normalize_folder(&path).map_err(|_| NO_HOME)?;
    if is_network_path(&local) {
        return Err(NO_HOME);
    }
    Ok(local)
}

/// OpenCode v2's global-roots.ts and database-path.ts: official release channels share
/// opencode.db. A relative OPENCODE_DB is relative to the CLI's data directory.
fn opencode_database(host: &Host) -> Option<String> {
    let db = host.opencode_db.as_deref().unwrap_or("opencode.db");
    if db.is_empty() || db == ":memory:" {
        return None;
    }
    if is_search_dir(db, host.windows) {
        return Some(db.to_owned());
    }
    if host.windows && (db.starts_with(['/', '\\']) || db.contains(':')) {
        return None;
    }
    let data = match host.xdg_data_home.as_deref().filter(|s| !s.is_empty()) {
        Some(root) => host.join(root, &["opencode"]),
        None => host.join(
            host.user_profile.as_deref()?,
            &[".local", "share", "opencode"],
        ),
    };
    Some(host.join(&data, &[db]))
}

#[cfg(test)]
mod tests;
