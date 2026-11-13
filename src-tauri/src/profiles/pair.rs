//! What the two merged files of a pair (the settings and the state file) have in common:
//! the base of the last sync, kept per profile in `<profile dir>\.traygent-base\<file>`,
//! and the careful write of both sides. A base holds the shared part only — nothing of the
//! sign-in is ever written there — and it exists in profile directories alone, outside
//! every shared directory: no rule links, syncs or backs it up.
use super::files::{
    load, unchanged, writable, write_atomic, write_stamped, Backups, Change, Loaded,
    CHANGED_MEANWHILE, MAX_FILE_BYTES,
};
use super::node::{same_object, to_file, Entries, Node};
use std::path::Path;
use std::time::SystemTime;

pub const BASE_DIR: &str = ".traygent-base";
const BASE_FAILED: &str = "Abgleichsstand konnte nicht gespeichert werden";

/// How a merged rule went for one pair.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Synced {
    /// The sides written — also when a later step failed.
    pub change: Change,
    /// A side is being written right now (an empty file): nothing was done, the rule has
    /// neither succeeded nor failed.
    pub skipped: bool,
    pub error: Option<String>,
    /// The rule went through, but had to leave parts alone: reported like errors, yet
    /// the rule has had its sync.
    pub partial: Vec<String>,
}

#[cfg(test)]
thread_local! {
    /// Runs after both sides were read: a test's stand-in for a session writing meanwhile.
    pub(super) static AFTER_READ: std::cell::RefCell<Option<Box<dyn Fn()>>> =
        const { std::cell::RefCell::new(None) };
}

/// Called by a rule once it has read both sides.
pub(super) fn sides_read() {
    #[cfg(test)]
    AFTER_READ.with(|hook| {
        if let Some(hook) = hook.borrow().as_ref() {
            hook();
        }
    });
}

/// A real directory or nothing: a link in its place could lead into another home.
fn base_dir_usable(dir: &Path) -> bool {
    match std::fs::symlink_metadata(dir) {
        Ok(meta) => meta.is_dir() && !meta.file_type().is_symlink(),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}

/// The shared part as of the last sync; `None` when there is none that can be read (also
/// one beyond the size cap).
pub fn read_base(profile_dir: &Path, name: &str) -> Option<Entries> {
    let dir = profile_dir.join(BASE_DIR);
    if !base_dir_usable(&dir) {
        return None;
    }
    let loaded = load(&dir.join(name), MAX_FILE_BYTES).ok()??;
    Node::parse_object(&loaded.bytes)
}

/// Records the base. A shared part beyond the size cap is recorded as an empty one: the
/// next sync then unites both sides and removes nothing.
fn write_base(profile_dir: &Path, name: &str, shared: &[(String, Node)]) -> Result<(), ()> {
    let dir = profile_dir.join(BASE_DIR);
    if !base_dir_usable(&dir) {
        return Err(());
    }
    let mut bytes = to_file(shared);
    let mut shared = shared;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        (bytes, shared) = (to_file(&[]), &[]);
    }
    if read_base(profile_dir, name).is_some_and(|base| same_object(&base, shared)) {
        return Ok(());
    }
    std::fs::create_dir_all(&dir).map_err(drop)?;
    write_atomic(&dir.join(name), &bytes).map_err(drop)
}

/// One side of a pair after the merge.
pub(super) struct Out<'a> {
    pub path: &'a Path,
    /// The file as it was read.
    pub seen: &'a Option<Loaded>,
    /// Its new content; `None` when it stays as it is.
    pub new: Option<Vec<u8>>,
    /// The merge takes something from this side: its file goes to the backup first.
    pub backup: bool,
}

/// Writes the sides that changed (the default home's first), then the base. Everything is
/// checked before the first write, so a refusal leaves both files alone: a link is never
/// written through, and a file a session wrote since it was read is not replaced — the
/// base then stays what it was and the next sync merges the new content. A new file
/// carries the time of the newest content merged into it, not the moment of the sync, so
/// it never looks newer than a third profile's own edit.
pub(super) fn commit(
    name: &str,
    outs: [Out; 2],
    profile_dir: &Path,
    shared: &[(String, Node)],
    backups: &mut Backups,
    change: &mut Change,
) -> Result<(), &'static str> {
    for out in outs.iter().filter(|out| out.new.is_some()) {
        writable(out.path)?;
        if !unchanged(out.path, out.seen.as_ref()) {
            return Err(CHANGED_MEANWHILE);
        }
    }
    let slots = [&mut *backups.default, &mut *backups.profile];
    for (out, slot) in outs.iter().zip(slots) {
        if let (Some(_), Some(seen), true) = (&out.new, out.seen, out.backup) {
            slot.keep(name, seen)?;
        }
    }
    let modified = |out: &Out| out.seen.as_ref().map(|seen| seen.modified);
    let time = outs.iter().filter_map(modified).max();
    let written = [&mut change.default, &mut change.profile];
    for (out, written) in outs.iter().zip(written) {
        let Some(bytes) = &out.new else {
            continue;
        };
        if !unchanged(out.path, out.seen.as_ref()) {
            return Err(CHANGED_MEANWHILE);
        }
        write_stamped(out.path, bytes, time)?;
        *written = true;
    }
    write_base(profile_dir, name, shared).map_err(|()| BASE_FAILED)
}

/// A side's file time; the epoch for a side without a file.
pub(super) fn modified(seen: &Option<Loaded>) -> SystemTime {
    seen.as_ref()
        .map_or(SystemTime::UNIX_EPOCH, |seen| seen.modified)
}
