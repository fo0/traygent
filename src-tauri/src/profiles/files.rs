//! File primitives of the profile sync: atomic writes, the backup folder and the synced
//! files (the newest wins, the default home on the first sync).
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A synced file beyond this is left alone.
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
pub const BACKUP_DIR: &str = ".traygent-backup";
const TOO_LARGE: &str = "ist zu groß";
const NOT_A_FILE: &str = "ist keine Datei";
const IS_A_LINK: &str = "ist eine Verknüpfung und wird nicht überschrieben";
pub(crate) const ACCESS_FAILED: &str = "Zugriff fehlgeschlagen";

/// Which side of a pair a rule wrote.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Change {
    pub default: bool,
    pub profile: bool,
}

impl Change {
    pub fn any(self) -> bool {
        self.default || self.profile
    }

    pub fn merge(&mut self, other: Change) {
        self.default |= other.default;
        self.profile |= other.profile;
    }
}

/// `<name>: <problem>` — one entry of a profile's `syncError`.
pub(crate) fn problem(name: &str, what: &str) -> String {
    format!("{name}: {what}")
}

/// A regular file's content and time; `None` when nothing is there.
pub(crate) struct Loaded {
    pub bytes: Vec<u8>,
    pub modified: SystemTime,
}

/// The content of a side, when there is one.
pub(crate) fn bytes(side: &Option<Loaded>) -> Option<&[u8]> {
    side.as_ref().map(|loaded| loaded.bytes.as_slice())
}

/// The text without a UTF-8 byte order mark (Windows PowerShell and some editors write
/// one; a JSON parser stops at it).
pub(crate) fn without_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
}

pub(crate) const CHANGED_MEANWHILE: &str =
    "wurde gerade geändert, nächster Versuch beim nächsten Abgleich";

/// Whether `path` still is what `seen` was read from (size and file time; missing stays
/// missing). The CLI writes its files while a session runs: a rule that merged an older
/// read must not replace what arrived since.
pub(crate) fn unchanged(path: &Path, seen: Option<&Loaded>) -> bool {
    match (std::fs::metadata(path), seen) {
        (Ok(meta), Some(seen)) => {
            meta.len() == seen.bytes.len() as u64
                && meta.modified().unwrap_or(SystemTime::UNIX_EPOCH) == seen.modified
        }
        (Err(error), None) => error.kind() == io::ErrorKind::NotFound,
        _ => false,
    }
}

/// Reads `path` when it is a file of at most `cap` bytes. A link is followed (a `CLAUDE.md`
/// kept in a dotfiles folder still syncs out); anything else that is no file is an error.
pub(crate) fn load(path: &Path, cap: u64) -> Result<Option<Loaded>, &'static str> {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // A dangling link is something, not nothing: never replace it.
            return match std::fs::symlink_metadata(path) {
                Ok(_) => Err(IS_A_LINK),
                Err(_) => Ok(None),
            };
        }
        Err(_) => return Err(ACCESS_FAILED),
    };
    if !meta.is_file() {
        return Err(NOT_A_FILE);
    }
    if meta.len() > cap {
        return Err(TOO_LARGE);
    }
    let bytes = std::fs::read(path).map_err(|_| ACCESS_FAILED)?;
    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    Ok(Some(Loaded { bytes, modified }))
}

/// A link is never written through ([`write_atomic`]).
pub(crate) fn writable(path: &Path) -> Result<(), &'static str> {
    if is_link(path) {
        return Err(IS_A_LINK);
    }
    Ok(())
}

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

/// Writes `bytes` to `path` through a temp file in the same directory and a rename, so a
/// reader never sees half a file. A link at `path` is refused: the rename would replace the
/// link, not the file behind it.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), &'static str> {
    write_stamped(path, bytes, None)
}

/// [`write_atomic`] with the file time `modified`. The temp file is stamped before the
/// rename, so whatever a session writes right after it keeps its own, newer time.
pub(crate) fn write_stamped(
    path: &Path,
    bytes: &[u8],
    modified: Option<SystemTime>,
) -> Result<(), &'static str> {
    use std::io::Write;
    if is_link(path) {
        return Err(IS_A_LINK);
    }
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(ACCESS_FAILED);
    };
    let mut temp_name = std::ffi::OsString::from(".traygent-tmp-");
    temp_name.push(std::process::id().to_string());
    temp_name.push("-");
    temp_name.push(name);
    let temp = dir.join(temp_name);
    let written = std::fs::File::create(&temp)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            if let Some(modified) = modified {
                let _ = file.set_modified(modified);
            }
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&temp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
        return Err(ACCESS_FAILED);
    }
    Ok(())
}

/// Copies a synced file's content and gives the copy the source's file time: the time says
/// how old the content is, not when it arrived, so the copy never looks newer than a third
/// profile's own edit.
fn copy_to(path: &Path, source: &Loaded) -> Result<(), &'static str> {
    write_stamped(path, &source.bytes, Some(source.modified))
}

/// The folder one sync run moves replaced data into:
/// `<profile dir>\.traygent-backup\<UTC yyyymmddThhmmssZ>`, created on first use.
pub struct Backup {
    root: PathBuf,
    stamp: String,
    dir: Option<PathBuf>,
}

impl Backup {
    /// The backup folder inside `home`: a profile's directory or the default home. What a
    /// rule replaces stays in the home it came from, so no identity crosses profiles.
    pub fn new(home: &Path, now: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            root: home.join(BACKUP_DIR),
            stamp: now.format("%Y%m%dT%H%M%SZ").to_string(),
            dir: None,
        }
    }

    /// Where `name` goes; the run's folder is new, so nothing in it is ever replaced.
    pub(crate) fn slot(&mut self, name: &str) -> io::Result<PathBuf> {
        if self.dir.is_none() {
            std::fs::create_dir_all(&self.root)?;
            let mut dir = self.root.join(&self.stamp);
            let mut counter = 1;
            // A second run in the same second gets its own folder.
            while std::fs::create_dir(&dir).is_err() {
                counter += 1;
                if counter > 100 {
                    return Err(io::ErrorKind::AlreadyExists.into());
                }
                dir = self.root.join(format!("{}-{counter}", self.stamp));
            }
            self.dir = Some(dir);
        }
        let dir = self.dir.as_ref().ok_or(io::ErrorKind::NotFound)?;
        // Two losers of one name in one run (two profiles replacing the default home's
        // file) each keep their own slot.
        let mut slot = dir.join(name);
        let mut counter = 1;
        while std::fs::symlink_metadata(&slot).is_ok() {
            counter += 1;
            if counter > 100 {
                return Err(io::ErrorKind::AlreadyExists.into());
            }
            slot = dir.join(format!("{name}.{counter}"));
        }
        Ok(slot)
    }

    /// Copies `loser` into a slot before a rule replaces it.
    pub(crate) fn keep(&mut self, name: &str, loser: &Loaded) -> Result<(), &'static str> {
        let slot = self.slot(name).map_err(|_| ACCESS_FAILED)?;
        copy_to(&slot, loser)
    }

    /// Hands a moved directory back to `home` and drops the run's folder when that leaves
    /// it empty (`remove_dir` only ever removes an empty directory).
    pub(crate) fn give_back(&mut self, slot: &Path, home: &Path) -> io::Result<()> {
        std::fs::rename(slot, home)?;
        if let Some(dir) = self.dir.take() {
            if std::fs::remove_dir(&dir).is_err() {
                self.dir = Some(dir);
            } else {
                let _ = std::fs::remove_dir(&self.root);
            }
        }
        Ok(())
    }
}

/// The backup folders of a pair: what a rule replaces goes to its own side's folder.
pub struct Backups<'a> {
    pub default: &'a mut Backup,
    pub profile: &'a mut Backup,
}

/// One synced file of a pair. Until the rule's first sync succeeded (`first`) the default
/// home's file wins; later the newer one wins in either direction. The file that loses is
/// copied to its side's backup first, whichever side it is. Equal content writes nothing; a
/// file on one side only is copied to the other.
pub fn sync_file(
    name: &str,
    default: &Path,
    profile: &Path,
    first: bool,
    backups: &mut Backups,
) -> Result<Change, String> {
    let fail = |what: &str| problem(name, what);
    let ours = load(default, MAX_FILE_BYTES).map_err(fail)?;
    let theirs = load(profile, MAX_FILE_BYTES).map_err(fail)?;
    let to_profile = Change {
        profile: true,
        ..Change::default()
    };
    let to_default = Change {
        default: true,
        ..Change::default()
    };
    match (ours, theirs) {
        (None, None) => Ok(Change::default()),
        (Some(ours), None) => {
            copy_to(profile, &ours).map_err(fail)?;
            Ok(to_profile)
        }
        (None, Some(theirs)) => {
            copy_to(default, &theirs).map_err(fail)?;
            Ok(to_default)
        }
        (Some(ours), Some(theirs)) if ours.bytes == theirs.bytes => Ok(Change::default()),
        (Some(ours), Some(theirs)) => {
            // A tie goes to the default home.
            if first || ours.modified >= theirs.modified {
                // Checked before the backup, so a refused write leaves no backup behind.
                writable(profile).map_err(fail)?;
                backups.profile.keep(name, &theirs).map_err(fail)?;
                copy_to(profile, &ours).map_err(fail)?;
                Ok(to_profile)
            } else {
                writable(default).map_err(fail)?;
                backups.default.keep(name, &ours).map_err(fail)?;
                copy_to(default, &theirs).map_err(fail)?;
                Ok(to_default)
            }
        }
    }
}
