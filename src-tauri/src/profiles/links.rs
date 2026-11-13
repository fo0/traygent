//! The shared directories of a profile: each is a directory link to the default home's
//! directory of the same name — a junction on Windows (no admin rights or developer mode
//! needed), a symbolic link elsewhere.
use super::files::{problem, Backup, ACCESS_FAILED};
use crate::detect::{Host, Runner, SystemRunner};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

const ELSEWHERE: &str = "Verknüpfung zeigt auf ein anderes Ziel";
const NOT_A_DIR: &str = "ist kein Ordner";
const DEFAULT_NOT_A_DIR: &str = "ist im Standard-Profil kein Ordner";
const LINK_FAILED: &str = "Verknüpfung konnte nicht angelegt werden";
const ADOPT_FAILED: &str = "Inhalt konnte nicht übernommen werden";
/// Windows PowerShell start plus a few `New-Item` calls.
pub const LINK_TIMEOUT: Duration = Duration::from_secs(20);
/// The env var the link list travels in; never part of the command line (docs/adr/0003).
pub const LINKS_ENV: &str = "TRAYGENT_LINKS";
/// Windows caps one environment variable at 32767 characters.
const MAX_PAYLOAD_CHARS: usize = 24_000;

/// A directory link to create: `link` does not exist, `target` is a directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub link: PathBuf,
    pub target: PathBuf,
}

/// Creates directory links, best effort: the caller checks every link afterwards.
pub trait Linker: Sync {
    fn link(&self, links: &[Link]);
}

/// Symbolic links, where they need no privilege.
#[cfg(unix)]
pub struct SymlinkLinker;

#[cfg(unix)]
impl Linker for SymlinkLinker {
    fn link(&self, links: &[Link]) {
        for pair in links {
            let _ = std::os::unix::fs::symlink(&pair.target, &pair.link);
        }
    }
}

/// Creates every junction of the list: pairs of lines, link then target, Base64 in
/// [`LINKS_ENV`]. One failing pair does not stop the others.
/// Windows PowerShell 5.1; no double quote anywhere (docs/adr/0003).
const LINK_SCRIPT: &str = r"$ErrorActionPreference = 'Stop'
$lines = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($env:TRAYGENT_LINKS)).Split([char]10)
for ($i = 0; $i + 1 -lt $lines.Length; $i += 2) {
  try { New-Item -ItemType Junction -Path $lines[$i] -Target $lines[$i + 1] | Out-Null } catch { }
}";

/// Directory junctions through one hidden Windows PowerShell call per list (std has no
/// junction API). The paths travel in an env var, never in the command text.
pub struct JunctionLinker {
    pub powershell: String,
}

/// The PowerShell argv; the script travels as `-EncodedCommand`.
pub fn junction_args() -> Vec<String> {
    let script = crate::launch::quote::encode_command(LINK_SCRIPT);
    let flags = ["-NoProfile", "-NonInteractive", "-EncodedCommand"];
    flags
        .into_iter()
        .map(String::from)
        .chain([script])
        .collect()
}

/// The [`LINKS_ENV`] values for `links`, usually one. A path that is not Unicode or holds a
/// control character is left out (its link then simply stays missing).
pub fn junction_payloads(links: &[Link]) -> Vec<String> {
    let text = |path: &Path| {
        let text = path.to_str()?;
        (!text.chars().any(char::is_control)).then(|| text.to_string())
    };
    let mut payloads = Vec::new();
    let mut lines = String::new();
    for pair in links {
        let (Some(link), Some(target)) = (text(&pair.link), text(&pair.target)) else {
            continue;
        };
        let entry = format!("{link}\n{target}\n");
        // Base64 grows the text by a third.
        if !lines.is_empty() && (lines.len() + entry.len()) * 4 / 3 > MAX_PAYLOAD_CHARS {
            payloads.push(crate::launch::quote::base64(lines.as_bytes()));
            lines.clear();
        }
        lines.push_str(&entry);
    }
    if !lines.is_empty() {
        payloads.push(crate::launch::quote::base64(lines.as_bytes()));
    }
    payloads
}

impl JunctionLinker {
    /// Only an absolute `powershell.exe` is ever run: a bare name would resolve against the
    /// working directory and PATH. Without one nothing is linked and every link reports
    /// that it could not be created.
    pub fn runnable(&self) -> bool {
        crate::launch::is_windows_absolute(&self.powershell)
    }
}

impl Linker for JunctionLinker {
    fn link(&self, links: &[Link]) {
        if !self.runnable() {
            return;
        }
        let args = junction_args();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        for payload in junction_payloads(links) {
            let runner = SystemRunner {
                timeout: LINK_TIMEOUT,
                env: vec![(LINKS_ENV.into(), payload)],
                ..SystemRunner::default()
            };
            let _ = runner.run(&self.powershell, &args);
        }
    }
}

/// The host's linker: junctions on Windows, symbolic links elsewhere.
pub fn system_linker(host: &Host) -> Box<dyn Linker> {
    #[cfg(unix)]
    {
        let _ = host;
        Box::new(SymlinkLinker)
    }
    #[cfg(not(unix))]
    {
        Box::new(JunctionLinker {
            powershell: host.powershell(),
        })
    }
}

/// What sits where a shared directory's link belongs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Found {
    Missing,
    /// A link (junction or symbolic) that resolves to the target.
    Linked,
    /// A link that resolves elsewhere, or nowhere.
    Elsewhere,
    /// A directory of the profile's own.
    Directory,
    /// A file or anything else.
    Other,
}

/// Whether two existing paths resolve to the same place. An error is no answer: a caller
/// that guards a move with this must refuse, not go on.
pub(crate) fn same_place(first: &Path, second: &Path) -> io::Result<bool> {
    Ok(std::fs::canonicalize(first)? == std::fs::canonicalize(second)?)
}

pub fn inspect(link: &Path, target: &Path) -> io::Result<Found> {
    let meta = match std::fs::symlink_metadata(link) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Found::Missing),
        Err(error) => return Err(error),
    };
    // `is_symlink` is true for a junction too (a name-surrogate reparse point).
    Ok(if meta.file_type().is_symlink() {
        // A link that resolves nowhere is one that points elsewhere: it is never touched.
        if same_place(link, target).unwrap_or(false) {
            Found::Linked
        } else {
            Found::Elsewhere
        }
    } else if meta.is_dir() {
        Found::Directory
    } else {
        Found::Other
    })
}

/// Copies every entry of `from` that `into` lacks, recursively. An existing file is never
/// overwritten, a link is never followed (it stays in the backup with the rest).
pub(crate) fn copy_missing(from: &Path, into: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let (source, dest) = (entry.path(), into.join(entry.file_name()));
        let existing = match std::fs::symlink_metadata(&dest) {
            Ok(meta) => Some(meta),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            match existing {
                None => {
                    std::fs::create_dir(&dest)?;
                    copy_missing(&source, &dest)?;
                }
                Some(meta) if meta.is_dir() => copy_missing(&source, &dest)?,
                Some(_) => {}
            }
        } else if kind.is_file() && existing.is_none() {
            copy_new(&source, &dest)?;
        }
    }
    Ok(())
}

/// Copies to a temp name beside `dest`, keeps the file time (session lists sort by it) and
/// the read-only flag (git pack files in `plugins`), then renames into place. The temp file
/// is written through one handle and stays writable until it has its name, so neither the
/// time step nor a cleanup trips over a read-only copy.
pub(crate) fn copy_new(source: &Path, dest: &Path) -> io::Result<()> {
    let mut temp_name = std::ffi::OsString::from(".traygent-tmp-");
    temp_name.push(std::process::id().to_string());
    temp_name.push("-");
    temp_name.push(dest.file_name().unwrap_or_default());
    let temp = dest.with_file_name(temp_name);
    let meta = std::fs::metadata(source)?;
    let copied = (|| {
        let mut from = std::fs::File::open(source)?;
        let mut into = std::fs::File::create(&temp)?;
        io::copy(&mut from, &mut into)?;
        // Best effort: a file system without settable times still gets the content.
        if let Ok(modified) = meta.modified() {
            let _ = into.set_modified(modified);
        }
        into.sync_all()?;
        drop(into);
        std::fs::rename(&temp, dest)
    })();
    match copied {
        Ok(()) => {
            if meta.permissions().readonly() {
                let _ = std::fs::set_permissions(dest, meta.permissions());
            }
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temp);
            Err(error)
        }
    }
}

/// A link to create and how to undo the room made for it should the link fail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub link: Link,
    /// The profile's own directory, moved to this backup slot.
    moved: Option<PathBuf>,
    /// An empty directory was removed.
    removed: bool,
}

impl Pending {
    /// The link could not be created: the profile gets its own directory back, so the CLI
    /// finds its data and the next sync starts from the same state instead of adopting (and
    /// backing up) a directory the CLI made anew.
    pub fn undo(&self, backup: &mut Backup) {
        if std::fs::symlink_metadata(&self.link.link).is_ok() {
            return;
        }
        if let Some(slot) = &self.moved {
            let _ = backup.give_back(slot, &self.link.link);
        } else if self.removed {
            let _ = std::fs::create_dir(&self.link.link);
        }
    }
}

/// Makes room for the link of the shared directory `name` and says whether one is to be
/// created: the default home's directory is created when missing; an empty directory in the
/// profile is removed; a non-empty one first hands every entry the default home lacks over,
/// then moves into the backup. A link is never touched — one that points elsewhere is an error.
pub fn prepare(
    name: &str,
    target: &Path,
    link: &Path,
    backup: &mut Backup,
) -> Result<Option<Pending>, String> {
    let fail = |what: &str| problem(name, what);
    match std::fs::metadata(target) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Err(fail(DEFAULT_NOT_A_DIR)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // A dangling link in the default home is the user's; never replace it.
            if std::fs::symlink_metadata(target).is_ok() {
                return Err(fail(DEFAULT_NOT_A_DIR));
            }
            std::fs::create_dir(target).map_err(|_| fail(ACCESS_FAILED))?;
        }
        Err(_) => return Err(fail(ACCESS_FAILED)),
    }
    let wanted = |moved: Option<PathBuf>, removed: bool| Pending {
        link: Link {
            link: link.to_path_buf(),
            target: target.to_path_buf(),
        },
        moved,
        removed,
    };
    match inspect(link, target).map_err(|_| fail(ACCESS_FAILED))? {
        Found::Linked => Ok(None),
        Found::Missing => Ok(Some(wanted(None, false))),
        Found::Elsewhere => Err(fail(ELSEWHERE)),
        Found::Other => Err(fail(NOT_A_DIR)),
        Found::Directory => {
            // The profile's directory *is* the default home's: nothing to adopt, and moving
            // it away would empty the default home. No answer is a refusal too.
            if same_place(link, target).map_err(|_| fail(ACCESS_FAILED))? {
                return Err(fail(ELSEWHERE));
            }
            let empty = std::fs::read_dir(link)
                .map(|mut entries| entries.next().is_none())
                .map_err(|_| fail(ACCESS_FAILED))?;
            if empty {
                // `remove_dir` only ever removes an empty directory.
                std::fs::remove_dir(link).map_err(|_| fail(ACCESS_FAILED))?;
                return Ok(Some(wanted(None, true)));
            }
            copy_missing(link, target).map_err(|_| fail(ADOPT_FAILED))?;
            let slot = backup.slot(name).map_err(|_| fail(ADOPT_FAILED))?;
            std::fs::rename(link, &slot).map_err(|_| fail(ADOPT_FAILED))?;
            Ok(Some(wanted(Some(slot), false)))
        }
    }
}

/// After the linker ran: the link exists and resolves to its target, or the error to report.
pub fn verify(name: &str, pair: &Link) -> Result<(), String> {
    match inspect(&pair.link, &pair.target) {
        Ok(Found::Linked) => Ok(()),
        _ => Err(problem(name, LINK_FAILED)),
    }
}
