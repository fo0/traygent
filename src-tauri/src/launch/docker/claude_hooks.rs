//! Projects the selected profile's user `settings.json` for a Docker Claude session (#55):
//! command hooks in exec form (`command` + `args`) that start Windows Node.js get a Linux
//! command, and their absolute path arguments under a mounted directory get the container
//! path. Everything else stays as it is. The launch script runs this as a private CLI
//! ([`CLI_ARG`]) and mounts the result over the original; the original is only read.
use crate::profiles::files::MAX_FILE_BYTES;
use crate::profiles::node::{set, Entries, Node};
use serde_json::Value;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Path;

/// `--docker-claude-hooks <selected config dir> <standard config dir or selected>
/// <Windows workspace> <Linux workspace> <output file> <1|0>`.
pub const CLI_ARG: &str = "--docker-claude-hooks";
/// Where the selected profile's config directory is mounted.
pub const CLAUDE_HOME: &str = "/home/node/.claude";
/// The container's own Node.js, found on PATH.
pub const NATIVE_NODE: &str = "node";
/// Starts the exact Node.js version given as its first argument (mise).
pub const NODE_HOOK: &str = "/opt/traygent/node-hook";
const SETTINGS_FILE: &str = "settings.json";
/// Node options whose next argument is code, never a path.
const CODE_FLAGS: [&str; 5] = ["-e", "--eval", "-p", "--print", "-pe"];

const USAGE: &str = "Aufruf: --docker-claude-hooks <Profilordner> <Standardordner> <Windows-Arbeitsordner> <Linux-Arbeitsordner> <Ausgabedatei> <1|0>";
const READ_FAILED: &str = "Die Claude-Einstellungen (settings.json) konnten nicht gelesen werden.";
const TOO_LARGE: &str = "Die Claude-Einstellungen (settings.json) sind zu groß.";
const NOT_AN_OBJECT: &str =
    "Die Claude-Einstellungen (settings.json) sind kein gültiges JSON-Objekt.";
const WRITE_FAILED: &str = "Die Hook-Einstellungen für Docker konnten nicht geschrieben werden.";
const SAME_FILE: &str = "Die Ausgabedatei darf nicht die Claude-Einstellungen selbst sein.";

/// A Windows path with a drive letter (`C:\…` or `C:/…`), as its drive and components.
/// `.` and `..` components make it no path to map: only a plain path is genuine.
#[derive(Clone, Debug)]
struct WindowsPath {
    drive: u8,
    parts: Vec<String>,
}

impl WindowsPath {
    fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let absolute = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'\\' | b'/');
        if !absolute {
            return None;
        }
        let mut parts = Vec::new();
        for part in text[3..].split(['\\', '/']) {
            match part {
                "" => {}
                "." | ".." => return None,
                _ if part.contains('\0') => return None,
                _ => parts.push(part.to_string()),
            }
        }
        Some(Self {
            drive: bytes[0].to_ascii_lowercase(),
            parts,
        })
    }

    /// The components below `root`, compared case-insensitively as Windows does.
    fn below(&self, root: &WindowsPath) -> Option<&[String]> {
        let same = |a: &String, b: &String| a == b || a.to_lowercase() == b.to_lowercase();
        let inside = self.drive == root.drive
            && self.parts.len() >= root.parts.len()
            && root.parts.iter().zip(&self.parts).all(|(a, b)| same(a, b));
        inside.then(|| &self.parts[root.parts.len()..])
    }

    fn same(&self, other: &WindowsPath) -> bool {
        self.below(other).is_some_and(|rest| rest.is_empty())
    }
}

/// The host directories a Docker Claude session mounts, and where.
#[derive(Clone, Debug)]
pub struct Roots {
    selected: WindowsPath,
    /// The Standard home behind a named profile; `None` for the Standard profile itself.
    standard: Option<WindowsPath>,
    workspace: WindowsPath,
    linux_workspace: String,
}

impl Roots {
    pub fn new(
        selected: &str,
        standard: &str,
        workspace: &str,
        linux_workspace: &str,
    ) -> Result<Self, String> {
        let windows = |text: &str, what: &str| {
            WindowsPath::parse(text).ok_or_else(|| format!("Ungültiger {what}."))
        };
        let selected = windows(selected, "Claude-Profilordner")?;
        let standard = windows(standard, "Claude-Standardordner")?;
        let workspace = windows(workspace, "Windows-Arbeitsordner")?;
        let linux = linux_workspace.trim_end_matches('/');
        let valid = linux.starts_with('/')
            && !linux.contains(['\0', '\\'])
            && linux[1..]
                .split('/')
                .all(|part| !matches!(part, "" | "." | ".."));
        if !valid {
            return Err("Ungültiger Linux-Arbeitsordner.".into());
        }
        Ok(Self {
            standard: (!standard.same(&selected)).then_some(standard),
            selected,
            workspace,
            linux_workspace: linux.to_string(),
        })
    }

    /// The container path of a genuine Windows path under a mounted directory. A path in
    /// the Standard home of a named profile maps into the profile's own home only, so a
    /// hook never reaches another account's files.
    fn map(&self, arg: &str) -> Option<String> {
        let path = WindowsPath::parse(arg)?;
        let join = |base: &str, rest: &[String]| match rest {
            [] => base.to_string(),
            _ => format!("{base}/{}", rest.join("/")),
        };
        if let Some(rest) = path.below(&self.selected) {
            return Some(join(CLAUDE_HOME, rest));
        }
        if let Some(rest) = self.standard.as_ref().and_then(|root| path.below(root)) {
            return Some(join(CLAUDE_HOME, rest));
        }
        let rest = path.below(&self.workspace)?;
        Some(join(&self.linux_workspace, rest))
    }
}

/// How a hook starts Node.js: `Some(version)` for an exact `vX.Y.Z` install directory.
fn node_command(command: &str) -> Option<Option<String>> {
    let bare =
        |name: &str| name.eq_ignore_ascii_case("node") || name.eq_ignore_ascii_case("node.exe");
    if bare(command) {
        return Some(None);
    }
    let path = WindowsPath::parse(command)?;
    let (file, dirs) = path.parts.split_last()?;
    bare(file).then(|| dirs.last().and_then(|dir| exact_version(dir)))
}

/// `24.21.0` from `v24.21.0`: three plain numbers, no leading zeros.
fn exact_version(dir: &str) -> Option<String> {
    let version = dir.strip_prefix(['v', 'V'])?;
    let number = |part: &str| {
        (1..=9).contains(&part.len())
            && part.bytes().all(|byte| byte.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'))
    };
    let parts: Vec<&str> = version.split('.').collect();
    (parts.len() == 3 && parts.iter().all(|part| number(part))).then(|| version.to_string())
}

/// The arguments with every genuine path mapped; code after `-e` and its kin stays as is.
fn map_args(args: &[String], roots: &Roots) -> Vec<String> {
    let mut script = false;
    let mut code = false;
    let mut mapped = Vec::with_capacity(args.len());
    for arg in args {
        if code {
            code = false;
            mapped.push(arg.clone());
        } else if !script && CODE_FLAGS.contains(&arg.as_str()) {
            code = true;
            mapped.push(arg.clone());
        } else {
            script |= !arg.starts_with('-');
            mapped.push(roots.map(arg).unwrap_or_else(|| arg.clone()));
        }
    }
    mapped
}

fn text(value: &str) -> Node {
    Node::Leaf(Value::String(value.to_string()))
}

/// One hook handler: rewritten in place when it is a Node.js command hook in exec form.
fn project_handler(
    handler: &mut Entries,
    roots: &Roots,
    mise: bool,
    pins: &mut BTreeSet<String>,
) -> bool {
    let field = |key: &str| handler.iter().find(|(name, _)| name == key).map(|e| &e.1);
    if field("type").and_then(Node::as_str) != Some("command") {
        return false;
    }
    let Some(command) = field("command").and_then(Node::as_str) else {
        return false;
    };
    let Some(version) = node_command(command) else {
        return false;
    };
    let Some(args) = field("args").and_then(Node::as_strings) else {
        return false;
    };
    let args: Vec<String> = args.into_iter().map(str::to_string).collect();
    let mut new_args = map_args(&args, roots);
    let new_command = match version.filter(|_| mise) {
        Some(version) => {
            pins.insert(version.clone());
            new_args.insert(0, version);
            NODE_HOOK
        }
        None => NATIVE_NODE,
    };
    if new_command == command && new_args == args {
        return false;
    }
    set(handler, "command", text(new_command));
    set(
        handler,
        "args",
        Node::Array(new_args.iter().map(|arg| text(arg)).collect()),
    );
    true
}

/// The objects of an array value; anything else has none.
fn objects(node: &mut Node) -> impl Iterator<Item = &mut Entries> {
    let items = match node {
        Node::Array(items) => items.as_mut_slice(),
        _ => &mut [],
    };
    items.iter_mut().filter_map(|item| match item {
        Node::Object(entries) => Some(entries),
        _ => None,
    })
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Projection {
    settings: Vec<u8>,
    pins: BTreeSet<String>,
}

/// The projected settings and exact runtimes to warm before Claude starts its hook timers.
fn project(settings: &[u8], roots: &Roots, mise: bool) -> Result<Option<Projection>, String> {
    let mut entries = Node::parse_object(settings).ok_or(NOT_AN_OBJECT)?;
    let mut changed = false;
    let mut pins = BTreeSet::new();
    for (key, events) in entries.iter_mut() {
        let Node::Object(events) = events else {
            continue;
        };
        if key != "hooks" {
            continue;
        }
        for (_, groups) in events.iter_mut() {
            for group in objects(groups) {
                for (name, handlers) in group.iter_mut() {
                    if name != "hooks" {
                        continue;
                    }
                    for handler in objects(handlers) {
                        changed |= project_handler(handler, roots, mise, &mut pins);
                    }
                }
            }
        }
    }
    Ok(changed.then(|| Projection {
        settings: crate::profiles::node::to_file(&entries),
        pins,
    }))
}

/// Reads `settings` (never writes it) and writes `output`: the projection, or nothing
/// when the file is missing or no hook needs it. Only version numbers reach stdout.
fn project_file(
    settings: &Path,
    output: &Path,
    roots: &Roots,
    mise: bool,
) -> Result<Vec<String>, String> {
    if std::fs::symlink_metadata(settings).is_ok_and(|m| !m.is_file()) {
        return Err(READ_FAILED.into());
    }
    if let (Ok(a), Ok(b)) = (settings.canonicalize(), output.canonicalize()) {
        if a == b {
            return Err(SAME_FILE.into());
        }
    }
    let projected = match std::fs::File::open(settings) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err(READ_FAILED.into()),
        Ok(file) => {
            let mut bytes = Vec::new();
            let read = file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes);
            read.map_err(|_| READ_FAILED)?;
            if bytes.len() as u64 > MAX_FILE_BYTES {
                return Err(TOO_LARGE.into());
            }
            project(&bytes, roots, mise)?
        }
    };
    let projection = projected.unwrap_or_default();
    std::fs::write(output, projection.settings).map_err(|_| WRITE_FAILED)?;
    Ok(projection.pins.into_iter().collect())
}

fn run(args: Vec<OsString>, stdout: &mut dyn Write) -> Result<(), String> {
    let args: Vec<String> = args
        .into_iter()
        .map(OsString::into_string)
        .collect::<Result<_, _>>()
        .map_err(|_| USAGE)?;
    let [selected, standard, workspace, linux, output, mise] = args.as_slice() else {
        return Err(USAGE.into());
    };
    let mise = match mise.as_str() {
        "1" => true,
        "0" => false,
        _ => return Err(USAGE.into()),
    };
    let roots = Roots::new(selected, standard, workspace, linux)?;
    let settings = Path::new(selected).join(SETTINGS_FILE);
    let pins = project_file(&settings, Path::new(output), &roots, mise)?;
    writeln!(stdout, "{}", pins.join(" ")).map_err(|_| WRITE_FAILED.to_string())
}

/// The private CLI: `None` unless `args` (without the program name) start with
/// [`CLI_ARG`]. Exit 2 preserves Claude's normal handling of an unreadable user file.
/// The message never quotes the settings.
pub fn handle(
    args: impl IntoIterator<Item = OsString>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    let mut args = args.into_iter();
    if args.next().as_deref() != Some(std::ffi::OsStr::new(CLI_ARG)) {
        return None;
    }
    Some(match run(args.collect(), stdout) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            if matches!(message.as_str(), READ_FAILED | NOT_AN_OBJECT | TOO_LARGE) {
                2
            } else {
                1
            }
        }
    })
}

#[cfg(test)]
mod tests;
