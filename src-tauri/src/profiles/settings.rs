//! The settings file of a pair (#35): a three-way merge of the shared part against its
//! state at the last sync, kept per profile in `<profile dir>\.traygent-base\<file>`. The
//! base holds the shared part only — no key of the sign-in is ever written there — and it
//! exists in profile directories alone, outside every shared directory: no rule links,
//! syncs or backs it up.
use super::files::{
    load, problem, unchanged, without_bom, writable, write_atomic, Backups, Change, Loaded,
    CHANGED_MEANWHILE, MAX_FILE_BYTES,
};
use super::merge::{join, loses, split, Merge, Split};
use super::node::{get, index, same_object, to_file, Entries, Node};
use super::SettingsRule;
use std::path::Path;
use std::time::SystemTime;

pub const BASE_DIR: &str = ".traygent-base";
const UNREADABLE: &str = "ist kein lesbares JSON-Objekt und bleibt unverändert";
const ENV_NO_OBJECT: &str = "„env“ ist kein Objekt und bleibt unverändert";
const BASE_FAILED: &str = "Abgleichsstand konnte nicht gespeichert werden";

/// How the rule went for one pair.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Synced {
    /// The sides written — also when a later step failed.
    pub change: Change,
    /// A side is being written right now (an empty file): nothing was done, the rule has
    /// neither succeeded nor failed.
    pub skipped: bool,
    pub error: Option<String>,
}

/// One side as read.
struct Side<'a> {
    path: &'a Path,
    seen: Option<Loaded>,
    doc: Option<Entries>,
    parts: Option<Split>,
}

impl<'a> Side<'a> {
    fn read(path: &'a Path, rule: &SettingsRule) -> Result<Self, &'static str> {
        let seen = load(path, MAX_FILE_BYTES)?;
        let doc = match &seen {
            Some(loaded) if !blank(without_bom(&loaded.bytes)) => {
                Some(Node::parse_object(&loaded.bytes).ok_or(UNREADABLE)?)
            }
            _ => None,
        };
        let parts = doc.as_ref().map(|doc| split(doc, rule));
        Ok(Self {
            path,
            seen,
            doc,
            parts,
        })
    }

    /// A file without content: the CLI is writing it.
    fn in_flight(&self) -> bool {
        let seen = self.seen.as_ref();
        seen.is_some_and(|seen| blank(without_bom(&seen.bytes)))
    }

    fn modified(&self) -> SystemTime {
        let seen = self.seen.as_ref();
        seen.map_or(SystemTime::UNIX_EPOCH, |seen| seen.modified)
    }

    /// The side's new content, when the merge changes it; a missing file is created from
    /// the shared part.
    fn merged(&self, shared: &[(String, Node)], rule: &SettingsRule) -> Option<Vec<u8>> {
        let joined = join(shared, self.parts.as_ref(), self.doc.as_deref(), rule);
        let same = self
            .doc
            .as_ref()
            .is_some_and(|doc| same_object(doc, &joined));
        (!same).then(|| to_file(&joined))
    }

    /// Whether the merge takes something from this side that it had.
    fn loses(&self, shared: &[(String, Node)]) -> bool {
        self.parts.as_ref().is_some_and(|parts| {
            loses(
                &Node::Object(parts.shared.clone()),
                &Node::Object(shared.to_vec()),
            )
        })
    }

    /// Replaces the file unless a session wrote it since the read. The new file carries
    /// `time` — the age of the newest content merged into it, not the moment of the sync —
    /// so it never looks newer than a third profile's own edit.
    fn write(&self, bytes: &[u8], time: SystemTime) -> Result<(), &'static str> {
        if !unchanged(self.path, self.seen.as_ref()) {
            return Err(CHANGED_MEANWHILE);
        }
        write_atomic(self.path, bytes)?;
        if let Ok(file) = std::fs::File::options().write(true).open(self.path) {
            let _ = file.set_modified(time);
        }
        Ok(())
    }
}

fn blank(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_whitespace)
}

/// A real directory or nothing: a link in its place could lead into another home.
fn base_dir_usable(dir: &Path) -> bool {
    match std::fs::symlink_metadata(dir) {
        Ok(meta) => meta.is_dir() && !meta.file_type().is_symlink(),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}

/// The shared part as of the last sync; `None` when there is none that can be read.
pub fn read_base(profile_dir: &Path, name: &str) -> Option<Entries> {
    let dir = profile_dir.join(BASE_DIR);
    if !base_dir_usable(&dir) {
        return None;
    }
    let loaded = load(&dir.join(name), MAX_FILE_BYTES).ok()??;
    Node::parse_object(&loaded.bytes)
}

fn write_base(profile_dir: &Path, name: &str, shared: &[(String, Node)]) -> Result<(), ()> {
    let dir = profile_dir.join(BASE_DIR);
    if !base_dir_usable(&dir) {
        return Err(());
    }
    if read_base(profile_dir, name).is_some_and(|base| same_object(&base, shared)) {
        return Ok(());
    }
    std::fs::create_dir_all(&dir).map_err(drop)?;
    write_atomic(&dir.join(name), &to_file(shared)).map_err(drop)
}

/// A side that holds none of the base's keys, the base having two or more: that file was
/// recreated or emptied, it did not remove key after key. Merging against the base would
/// take every shared setting from the other side — and, through the default home, from
/// every profile.
fn reset(base: &[(String, Node)], side: &[(String, Node)]) -> bool {
    let held = index(side);
    base.len() >= 2 && base.iter().all(|(key, _)| !held.contains_key(key.as_str()))
}

#[cfg(test)]
thread_local! {
    /// Runs after both sides were read: a test's stand-in for a session writing meanwhile.
    pub(super) static AFTER_READ: std::cell::RefCell<Option<Box<dyn Fn()>>> =
        const { std::cell::RefCell::new(None) };
}

/// The settings file of a pair. The shared part of both sides is merged against the base
/// of `profile_dir`; each side then is that merge plus its own keys. With a base the newer
/// file wins a conflict. Until the rule's first sync succeeded (`first`), whenever there is
/// no base and when a side was reset ([`reset`]), the default home wins and nothing is
/// removed — except, for a pair without a base, what `seed` (the base of another profile of
/// the agent) says the default home has removed already. A side is written only when its
/// content (not its formatting) changes, and goes to its backup first on the first sync,
/// when it loses a key or a list item, and when a conflict went against it. The base is
/// written last, after both sides.
pub fn sync_settings(
    rule: &SettingsRule,
    [default, profile]: [&Path; 2],
    profile_dir: &Path,
    first: bool,
    seed: Option<&[(String, Node)]>,
    backups: &mut Backups,
) -> Synced {
    let mut synced = Synced::default();
    let paths = [default, profile];
    if let Err(what) = run(rule, paths, profile_dir, first, seed, backups, &mut synced) {
        synced.error = Some(problem(rule.file, what));
    }
    synced
}

fn run(
    rule: &SettingsRule,
    paths: [&Path; 2],
    profile_dir: &Path,
    first: bool,
    seed: Option<&[(String, Node)]>,
    backups: &mut Backups,
    synced: &mut Synced,
) -> Result<(), &'static str> {
    let name = rule.file;
    let ours = Side::read(paths[0], rule)?;
    let theirs = Side::read(paths[1], rule)?;
    if ours.in_flight() || theirs.in_flight() {
        synced.skipped = true;
        return Ok(());
    }
    #[cfg(test)]
    AFTER_READ.with(|hook| hook.borrow().as_ref().map(|hook| hook()));
    let base = if first {
        None
    } else {
        read_base(profile_dir, name)
    };
    let (mut default_wins, mut conflict) = (true, false);
    let shared = match (&ours.parts, &theirs.parts) {
        (None, None) => return Ok(()),
        // A file that is not there has removed nothing.
        (Some(only), None) | (None, Some(only)) => only.shared.clone(),
        (Some(a), Some(b)) => {
            let was_reset = base
                .as_ref()
                .is_some_and(|base| reset(base, &a.shared) || reset(base, &b.shared));
            let (mut merge, base) = match (&base, seed) {
                (Some(_), _) | (None, None) if was_reset || base.is_none() => {
                    (Merge::new(rule, true), None)
                }
                (Some(base), _) => {
                    default_wins = ours.modified() >= theirs.modified();
                    (Merge::new(rule, default_wins), Some(base.as_slice()))
                }
                (None, seed) => (Merge::seeded(rule), seed),
            };
            let shared = merge.objects(base, &a.shared, &b.shared);
            conflict = !merge.conflicts.is_empty();
            shared
        }
    };
    // A shared `env` that is no object cannot hold a side's own variables: writing would
    // either drop them or record a base that side never has.
    let own_env = |side: &Side| {
        side.parts
            .as_ref()
            .is_some_and(|parts| !parts.own_env.is_empty())
    };
    let env = get(&shared, rule.env_key);
    if env.is_some_and(|env| env.as_object().is_none()) && (own_env(&ours) || own_env(&theirs)) {
        return Err(ENV_NO_OBJECT);
    }
    let new = [ours.merged(&shared, rule), theirs.merged(&shared, rule)];
    let sides = [&ours, &theirs];
    // Everything is checked before the first write, so a refusal leaves both files alone.
    for (side, _) in sides.iter().zip(&new).filter(|(_, new)| new.is_some()) {
        writable(side.path)?;
        if !unchanged(side.path, side.seen.as_ref()) {
            return Err(CHANGED_MEANWHILE);
        }
    }
    let lost = [conflict && !default_wins, conflict && default_wins];
    let slots = [&mut *backups.default, &mut *backups.profile];
    for (((side, new), lost), slot) in sides.iter().zip(&new).zip(lost).zip(slots) {
        if let (Some(_), Some(seen)) = (new, &side.seen) {
            if first || lost || side.loses(&shared) {
                slot.keep(name, seen)?;
            }
        }
    }
    let time = ours.modified().max(theirs.modified());
    if let Some(bytes) = &new[0] {
        ours.write(bytes, time)?;
        synced.change.default = true;
    }
    if let Some(bytes) = &new[1] {
        theirs.write(bytes, time)?;
        synced.change.profile = true;
    }
    write_base(profile_dir, name, &shared).map_err(|()| BASE_FAILED)?;
    Ok(())
}
