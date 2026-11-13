//! What the user shares on top of the recipe (#35): extra directories and files by name.
//! A name becomes part of a path inside a profile directory, so it is checked here — by the
//! settings before they are saved and again by the sync before anything is joined.
use super::Recipe;
use crate::models::ProfileShare;

/// Extra directories, and extra files, per agent.
pub const MAX_SHARED: usize = 16;
const MAX_NAME_CHARS: usize = 64;
/// Traygent's own files and folders in a profile directory start with this.
const OWN_PREFIX: &str = ".traygent";
const DEVICES: [&str; 4] = ["con", "prn", "aux", "nul"];
const NUMBERED_DEVICES: [&str; 2] = ["com", "lpt"];

/// Why a name cannot be shared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    /// Not a plain file or folder name.
    Invalid,
    /// The recipe already shares or protects it.
    Handled,
}

/// A Windows device name (`NUL`, `COM1`, …), with or without an extension.
fn is_device(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let numbered = NUMBERED_DEVICES.iter().any(|device| {
        stem.strip_prefix(device)
            .is_some_and(|rest| rest.len() == 1 && rest.chars().all(|c| c.is_ascii_digit()))
    });
    numbered || DEVICES.contains(&stem.as_str())
}

/// The names the recipe shares or syncs itself.
fn handled(recipe: &Recipe) -> impl Iterator<Item = &'static str> + '_ {
    recipe
        .shared_dirs
        .iter()
        .chain(recipe.synced_files)
        .copied()
        .chain(recipe.history)
        .chain(recipe.settings.as_ref().map(|rule| rule.file))
}

/// The state file's and the sign-in's name: no shared name may start with either
/// (`.claude.json.backup`, `.credentials.json.bak` carry the same identity).
fn protected(recipe: &Recipe) -> [&'static str; 2] {
    [recipe.state_file, recipe.credentials]
}

/// What decides whether a name can be shared, as data: the panel checks its copy against
/// `fixtures/profile-share-names.json`, which a test writes from this.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameLists {
    /// „wird bereits abgeglichen“: the recipe shares or syncs these.
    pub handled: Vec<&'static str>,
    /// „ist ungültig“: a name that starts with one of these.
    pub protected_prefixes: Vec<&'static str>,
    /// „ist ungültig“: these names.
    pub never_shared: Vec<&'static str>,
    /// „ist ungültig“: a name that starts with this (Traygent's own).
    pub own_prefix: &'static str,
    /// „ist ungültig“: a name whose part before the first dot is one of these, or one of
    /// `numberedDevices` followed by one digit.
    pub devices: Vec<&'static str>,
    pub numbered_devices: Vec<&'static str>,
    pub max_name_chars: usize,
    pub max_per_list: usize,
}

/// Every comparison in it ignores ASCII case.
pub fn name_lists(recipe: &Recipe) -> NameLists {
    NameLists {
        handled: handled(recipe).collect(),
        protected_prefixes: protected(recipe).to_vec(),
        never_shared: recipe.never_shared.to_vec(),
        own_prefix: OWN_PREFIX,
        devices: DEVICES.to_vec(),
        numbered_devices: NUMBERED_DEVICES.to_vec(),
        max_name_chars: MAX_NAME_CHARS,
        max_per_list: MAX_SHARED,
    }
}

/// `^[A-Za-z0-9._-]{1,64}$`, not `.` or `..`, no trailing dot, no device name, nothing of
/// Traygent's own, nothing that starts with the state file's or the sign-in's name and none
/// of the recipe's never-shared names — all [`NameError::Invalid`]; and nothing the recipe
/// handles already ([`NameError::Handled`]). Names are compared without case, as the file
/// system does.
pub fn check_name(recipe: &Recipe, name: &str) -> Result<(), NameError> {
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    let lower = name.to_ascii_lowercase();
    let starts = |prefix: &str| lower.starts_with(&prefix.to_ascii_lowercase());
    let same = |known: &&str| known.eq_ignore_ascii_case(name);
    if name.is_empty()
        || name.len() > MAX_NAME_CHARS
        || !name.chars().all(plain)
        || name.ends_with('.')
        || is_device(name)
        || starts(OWN_PREFIX)
        || protected(recipe).into_iter().any(starts)
        || recipe.never_shared.iter().any(same)
    {
        return Err(NameError::Invalid);
    }
    if handled(recipe).any(|known| known.eq_ignore_ascii_case(name)) {
        return Err(NameError::Handled);
    }
    Ok(())
}

/// What one agent's profiles share: the recipe's lists plus the user's extras that pass
/// the check. The sync takes every name from here.
#[derive(Debug, PartialEq, Eq)]
pub struct Sharing<'a> {
    pub recipe: &'a Recipe,
    /// Directories linked into the default home.
    pub dirs: Vec<&'a str>,
    /// Files copied whole, the newest winning.
    pub files: Vec<&'a str>,
    /// German, one entry per extra that was skipped; it never became part of a path.
    pub skipped: Vec<String>,
}

impl<'a> Sharing<'a> {
    pub fn of(recipe: &'a Recipe, share: &'a ProfileShare) -> Self {
        let mut sharing = Self {
            recipe,
            dirs: recipe.shared_dirs.to_vec(),
            files: recipe.synced_files.to_vec(),
            skipped: Vec::new(),
        };
        let mut seen: Vec<String> = Vec::new();
        for (list, is_dir) in [(&share.dirs, true), (&share.files, false)] {
            for (index, name) in list.iter().enumerate() {
                let lower = name.to_ascii_lowercase();
                let fine = index < MAX_SHARED
                    && check_name(recipe, name).is_ok()
                    && !seen.contains(&lower);
                if !fine {
                    // The name is untrusted: shown shortened and without control characters.
                    let shown: String = name
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(MAX_NAME_CHARS)
                        .collect();
                    sharing.skipped.push(format!("{shown}: {SKIPPED}"));
                    continue;
                }
                seen.push(lower);
                if is_dir {
                    sharing.dirs.push(name);
                } else {
                    sharing.files.push(name);
                }
            }
        }
        sharing
    }
}

pub const SKIPPED: &str = "ungültiger Name, wird nicht abgeglichen";
