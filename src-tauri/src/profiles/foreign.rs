//! Whose a profile directory is: a non-empty one that holds nothing of the agent's belongs
//! to something else and is never touched — by the sync or by a sign-in.
use super::links;
use super::marker::MARKER;
use super::pair::BASE_DIR;
use super::recipe;
use super::share::Sharing;
use crate::models::{AgentId, ProfileShare};
use std::path::Path;

/// An existing, non-empty directory without the CLI's state file, its sign-in or the marker
/// is someone else's (`.claude-mem`, `.claude-backup`): nothing in it is touched.
pub fn foreign_dir(agent: AgentId) -> String {
    format!("Profilordner gehört nicht zu {}", agent.label())
}

/// An existing directory that holds something but nothing of the agent's: none of its own
/// files, and not only links to the default home's shared directories (what a sync leaves
/// that was stopped before its marker) — the user's extra ones included.
pub(super) fn is_foreign(
    dir: &Path,
    sharing: &Sharing,
    default_dir: &Path,
) -> std::io::Result<bool> {
    let own = [
        sharing.recipe.state_file,
        sharing.recipe.credentials,
        MARKER,
    ];
    if own
        .iter()
        .any(|name| std::fs::symlink_metadata(dir.join(name)).is_ok())
    {
        return Ok(false);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        // Traygent's own: the merge bases of the settings and the state file.
        if name.to_str() == Some(BASE_DIR) {
            continue;
        }
        let shared = sharing
            .dirs
            .iter()
            .find(|shared| name.to_str() == Some(**shared));
        let ours = match shared {
            Some(shared) if entry.file_type()?.is_symlink() => {
                links::same_place(&entry.path(), &default_dir.join(shared)).unwrap_or(false)
            }
            _ => false,
        };
        if !ours {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether a session may sign in to `dir`: not when it exists and is someone else's, nor
/// when that cannot be told. A missing directory is fine. `share`: the agent's extras.
pub fn refuses_sign_in(
    agent: AgentId,
    dir: &Path,
    default_dir: &Path,
    share: &ProfileShare,
) -> bool {
    let Some(recipe) = recipe(agent) else {
        return true;
    };
    match std::fs::symlink_metadata(dir) {
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
        Ok(_) => is_foreign(dir, &Sharing::of(recipe, share), default_dir).unwrap_or(true),
    }
}
