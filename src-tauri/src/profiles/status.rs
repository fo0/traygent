//! What the panel shows about each profile: its directory, whether it is signed in, the
//! account the CLI recorded and the last sync.
use super::sync::read_marker;
use super::{recipe, Homes};
use crate::models::{
    AgentId, AgentProfile, AppSettings, ProfileStatus, DEFAULT_PROFILE, DEFAULT_PROFILE_LABEL,
};
use std::collections::HashMap;
use std::path::Path;

const MAX_TEXT_CHARS: usize = 128;
/// A state file beyond this is not read for the account.
const MAX_STATE_BYTES: u64 = 64 * 1024 * 1024;

/// (agent, profile id).
pub type Key = (AgentId, String);

/// The host facts of one profile; everything else of a [`ProfileStatus`] comes from the
/// settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub signed_in: bool,
    pub account: Option<String>,
    pub organization: Option<String>,
    pub synced_at: Option<String>,
}

#[derive(serde::Deserialize)]
struct State {
    #[serde(rename = "oauthAccount")]
    account: Option<Account>,
}

#[derive(serde::Deserialize)]
struct Account {
    #[serde(rename = "emailAddress")]
    email: Option<String>,
    #[serde(rename = "organizationName")]
    organization: Option<String>,
}

/// Text from a file the CLI wrote: control characters dropped, capped, blank → none.
fn shown(text: Option<String>) -> Option<String> {
    let text: String = text?
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_TEXT_CHARS)
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn account(state_file: &Path) -> Option<Account> {
    let small = std::fs::metadata(state_file).is_ok_and(|meta| meta.len() <= MAX_STATE_BYTES);
    let bytes = small.then(|| std::fs::read(state_file).ok()).flatten()?;
    serde_json::from_slice::<State>(super::files::without_bom(&bytes))
        .ok()?
        .account
}

/// Reads one profile. The credentials file is only checked for existence and size — its
/// content never leaves the disk.
pub fn read(homes: &Homes, agent: AgentId, id: &str) -> Facts {
    let (Some(recipe), Some(dir)) = (recipe(agent), homes.dir(agent, id)) else {
        return Facts::default();
    };
    let dir = Path::new(&dir);
    let signed_in = std::fs::metadata(dir.join(recipe.credentials))
        .is_ok_and(|meta| meta.is_file() && meta.len() > 0);
    let account = homes
        .state_file(agent, id)
        .and_then(|file| account(Path::new(&file)));
    let (email, organization) = match account {
        Some(account) => (account.email, account.organization),
        None => (None, None),
    };
    let synced_at = (id != DEFAULT_PROFILE)
        .then(|| read_marker(dir)?.synced_at)
        .flatten()
        .filter(|at| chrono::DateTime::parse_from_rfc3339(at).is_ok());
    Facts {
        signed_in,
        account: shown(email),
        organization: shown(organization),
        synced_at,
    }
}

/// Every profile of `agents`, the default one included.
pub fn read_all(
    homes: Option<&Homes>,
    agents: &[(AgentId, Vec<AgentProfile>)],
) -> HashMap<Key, Facts> {
    let mut facts = HashMap::new();
    let Some(homes) = homes else {
        return facts;
    };
    for (agent, profiles) in agents {
        let ids = profiles.iter().map(|profile| profile.id.as_str());
        for id in std::iter::once(DEFAULT_PROFILE).chain(ids) {
            facts.insert((*agent, id.to_string()), read(homes, *agent, id));
        }
    }
    facts
}

/// `Snapshot.profiles`: for every agent with profiles the default one („Standard“) first,
/// then the settings' order.
pub fn list(
    homes: Option<&Homes>,
    settings: &AppSettings,
    facts: &HashMap<Key, Facts>,
    errors: &HashMap<Key, String>,
) -> Vec<ProfileStatus> {
    let mut list = Vec::new();
    for agent in AgentId::ALL {
        if recipe(agent).is_none() {
            continue;
        }
        let standard = AgentProfile {
            id: DEFAULT_PROFILE.into(),
            label: DEFAULT_PROFILE_LABEL.into(),
        };
        for profile in std::iter::once(&standard).chain(settings.profiles.get(agent)) {
            let key = (agent, profile.id.clone());
            let known = facts.get(&key).cloned().unwrap_or_default();
            list.push(ProfileStatus {
                agent,
                id: profile.id.clone(),
                label: profile.label.clone(),
                dir: homes
                    .and_then(|homes| homes.dir(agent, &profile.id))
                    .unwrap_or_default(),
                signed_in: known.signed_in,
                account: known.account,
                organization: known.organization,
                synced_at: known.synced_at,
                sync_error: errors.get(&key).cloned(),
            });
        }
    }
    list
}
