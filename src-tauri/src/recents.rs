//! Recent sessions: one entry per (folder, agent, env), newest first, at most [`MAX_RECENTS`].
use crate::models::{AgentId, EnvId, LaunchRequest, RecentSession, RepoRef};
use crate::ports;
use chrono::{DateTime, SecondsFormat, Utc};
use sha2::{Digest, Sha256};

pub const MAX_RECENTS: usize = 20;

/// Stable id: the first 16 hex digits of SHA-256 over the lowercased folder, agent and env.
pub fn recent_id(folder: &str, agent: AgentId, env: EnvId) -> String {
    let key = format!("{}\n{}\n{}", folder.to_lowercase(), agent.id(), env.id());
    hex_prefix(&Sha256::digest(key.as_bytes()), 16)
}

pub(crate) fn hex_prefix(bytes: &[u8], digits: usize) -> String {
    bytes
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0x0f])
        .take(digits)
        .map(|nibble| char::from_digit(u32::from(nibble), 16).unwrap_or('0'))
        .collect()
}

pub fn timestamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn started(recent: &RecentSession) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&recent.last_started_at)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

/// Newest first; entries with an unreadable time sink to the end.
fn sort(list: &mut [RecentSession]) {
    list.sort_by_key(|recent| std::cmp::Reverse(started(recent)));
}

/// Records a start: bumps `starts` and `lastStartedAt`, replaces the options, ports and
/// profile (stored normalized), moves it first. A start of the same folder by path keeps the repository the
/// entry was cloned from.
pub fn upsert(
    list: &mut Vec<RecentSession>,
    request: &LaunchRequest,
    now: DateTime<Utc>,
) -> RecentSession {
    let id = recent_id(&request.folder, request.agent, request.env);
    let previous = list
        .iter()
        .position(|recent| recent.id == id)
        .map(|index| list.remove(index));
    let starts = previous.as_ref().map_or(0, |recent| recent.starts);
    let repo = stored_repo(request.repo.as_ref())
        .or_else(|| previous.and_then(|recent| stored_repo(recent.repo.as_ref())));
    let recent = RecentSession {
        id,
        agent: request.agent,
        env: request.env,
        folder: request.folder.clone(),
        repo,
        options: request.options.clone(),
        ports: stored_ports(request.env, &request.ports),
        profile: stored_profile(request.env, request.profile.as_deref()),
        last_started_at: timestamp(now),
        starts: starts.saturating_add(1),
    };
    list.insert(0, recent.clone());
    list.truncate(MAX_RECENTS);
    recent
}

pub fn remove(list: &mut Vec<RecentSession>, id: &str) -> Option<RecentSession> {
    let index = list.iter().position(|recent| recent.id == id)?;
    Some(list.remove(index))
}

/// Undo of [`remove`]: re-inserts the entry in time order. An entry started again since the
/// removal is newer and wins.
pub fn restore(list: &mut Vec<RecentSession>, recent: RecentSession) -> Result<(), String> {
    if started(&recent).is_none() || recent.starts == 0 {
        return Err("Eintrag kann nicht wiederhergestellt werden.".into());
    }
    let recent = RecentSession {
        id: recent_id(&recent.folder, recent.agent, recent.env),
        ..recent
    };
    if list.iter().all(|existing| existing.id != recent.id) {
        list.push(recent);
        sort(list);
        list.truncate(MAX_RECENTS);
    }
    Ok(())
}

/// Ports as a recent keeps them: normalized, Docker only; an invalid list is dropped, so a
/// tampered `state.json` never hands one to a restart.
pub fn stored_ports(env: EnvId, list: &[String]) -> Vec<String> {
    (env == EnvId::Docker)
        .then(|| ports::normalize_ports(list).ok())
        .flatten()
        .unwrap_or_default()
}

/// The profile as a recent keeps it: an id of an extra profile, local sessions only; the
/// default profile, another environment and a malformed id are stored as none. Whether the
/// profile still exists is checked at the start.
pub fn stored_profile(env: EnvId, profile: Option<&str>) -> Option<String> {
    profile
        .filter(|id| env == EnvId::Local && crate::profiles::is_valid_id(id))
        .map(str::to_string)
}

/// The repository as a recent keeps it: validated, an invalid one dropped (the folder still
/// starts).
pub fn stored_repo(repo: Option<&RepoRef>) -> Option<RepoRef> {
    repo.and_then(|repo| crate::repo::validate(repo).ok())
}

/// Loaded lists: ports ([`stored_ports`]), repositories ([`stored_repo`]) and profiles
/// ([`stored_profile`]) sanitized,
/// duplicates dropped (first wins after sorting) and the cap kept.
pub fn tidy(mut list: Vec<RecentSession>) -> Vec<RecentSession> {
    for recent in &mut list {
        recent.ports = stored_ports(recent.env, &recent.ports);
        recent.repo = stored_repo(recent.repo.as_ref());
        recent.profile = stored_profile(recent.env, recent.profile.as_deref());
    }
    sort(&mut list);
    let mut seen = std::collections::HashSet::new();
    list.retain(|recent| seen.insert(recent.id.clone()));
    list.truncate(MAX_RECENTS);
    list
}

#[cfg(test)]
mod tests;
