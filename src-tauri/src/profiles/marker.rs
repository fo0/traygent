//! The marker file that makes a profile directory Traygent's and remembers what its sync
//! has done.
use super::files::write_atomic;
use super::json::Base;
use std::path::Path;

pub const MARKER: &str = ".traygent-sync.json";
pub(super) const MARKER_VERSION: u64 = 1;

/// `<profile dir>\.traygent-sync.json`: `done` names the rules whose first sync succeeded —
/// until then the default home wins for that rule, afterwards the newest. `base` holds the
/// entry names (never values) of the state file's shared keys as of that rule's last
/// successful sync, so a removed entry is told from an added one. `syncedAt` is the last
/// sync without an error. A marker without `done` or `base` (an older one, or the initial
/// one of a new profile directory) only says the directory is Traygent's.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<String>,
    #[serde(default)]
    pub done: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Base>,
}

/// `None` without a marker file; an unreadable one still counts as a marker.
pub fn read_marker(dir: &Path) -> Option<Marker> {
    let bytes = std::fs::read(dir.join(MARKER)).ok()?;
    Some(serde_json::from_slice(&bytes).unwrap_or_default())
}

pub(super) fn write_marker(dir: &Path, marker: &Marker) -> Result<(), ()> {
    let bytes = serde_json::to_vec(marker).map_err(drop)?;
    write_atomic(&dir.join(MARKER), &bytes).map_err(drop)
}
