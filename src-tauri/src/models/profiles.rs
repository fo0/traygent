//! The agent-profile part of the IPC data model (#33), mirrored from `src/lib/types.ts`.
use super::AgentId;
use serde::{Deserialize, Serialize};

/// The id of the profile every agent has: the CLI's own home.
pub const DEFAULT_PROFILE: &str = "default";
/// The label of the default profile; reserved, no extra profile may carry it.
pub const DEFAULT_PROFILE_LABEL: &str = "Standard";

/// An extra profile of an agent: its own sign-in, everything else kept in sync.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfile {
    /// `^[a-z0-9][a-z0-9-]{0,23}$`, never `default`; names the profile directory.
    pub id: String,
    /// Trimmed, 1–24 characters, unique per agent (case-insensitive).
    pub label: String,
}

/// What Traygent knows about one profile on this host; read at detection, after a sync and
/// after a settings save.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStatus {
    pub agent: AgentId,
    /// `default` or an [`AgentProfile::id`].
    pub id: String,
    pub label: String,
    /// The profile's config directory; `''` when the host has no usable user profile.
    pub dir: String,
    /// The profile holds a non-empty credentials file (its content is never read).
    pub signed_in: bool,
    /// The signed-in account's e-mail address, when the CLI recorded one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    /// RFC 3339 time of the last sync without an error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<String>,
    /// German: what the last sync could not do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_error: Option<String>,
}
