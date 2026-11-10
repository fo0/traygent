//! Where CLIs live: the host's search dirs and the known install locations per tool
//! (docs/AGENT_RECIPES.md §0–§3). Paths are plain strings joined with the host's separator,
//! so the Windows rules are testable on any platform with a fake [`Fs`].
use crate::models::AgentId;

/// File-system questions the resolver asks, and the one directory a repository start creates;
/// tests answer them from a fixed set.
pub trait Fs: Sync {
    fn is_file(&self, path: &str) -> bool;
    fn is_dir(&self, path: &str) -> bool;
    /// Creates `path` and its missing parents (the clone root before a `git clone`).
    fn create_dir_all(&self, path: &str) -> std::io::Result<()>;
}

/// The real file system.
pub struct RealFs;

impl Fs for RealFs {
    fn is_file(&self, path: &str) -> bool {
        // App execution aliases (`WindowsApps\wt.exe`) are reparse points that `metadata`
        // may fail to follow; they still start.
        match std::fs::metadata(path) {
            Ok(meta) => meta.is_file(),
            Err(_) => std::fs::symlink_metadata(path).is_ok_and(|meta| !meta.is_dir()),
        }
    }

    fn is_dir(&self, path: &str) -> bool {
        std::path::Path::new(path).is_dir()
    }

    fn create_dir_all(&self, path: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }
}

/// A found executable; `shim` marks an npm `.cmd` that re-parses its args through cmd.exe.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Candidate {
    pub path: String,
    pub shim: bool,
}

/// The host facts resolution needs, captured once per detection or launch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Host {
    pub windows: bool,
    /// Process PATH, then (Windows) the registry User and Machine PATH, deduplicated.
    pub path_dirs: Vec<String>,
    pub user_profile: Option<String>,
    pub app_data: Option<String>,
    pub local_app_data: Option<String>,
    pub program_files: Option<String>,
    pub system_root: Option<String>,
    pub codex_install_dir: Option<String>,
}

impl Host {
    pub fn current() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let windows = cfg!(windows);
        let mut dirs = split_path(var("PATH").as_deref().unwrap_or(""), windows);
        dirs.extend(registry_path_dirs());
        Self {
            windows,
            path_dirs: dedup(dirs, windows),
            user_profile: var(if windows { "USERPROFILE" } else { "HOME" }),
            app_data: var("APPDATA"),
            local_app_data: var("LOCALAPPDATA"),
            program_files: var("ProgramFiles"),
            system_root: var("SystemRoot"),
            codex_install_dir: var("CODEX_INSTALL_DIR"),
        }
    }

    fn sep(&self) -> char {
        if self.windows {
            '\\'
        } else {
            '/'
        }
    }

    /// `base` plus the parts, with exactly one separator between them.
    pub fn join(&self, base: &str, parts: &[&str]) -> String {
        let sep = self.sep();
        let mut path = base.trim_end_matches(['\\', '/']).to_string();
        for part in parts {
            path.push(sep);
            path.push_str(part);
        }
        path
    }

    fn exe(&self, name: &str) -> String {
        if self.windows {
            format!("{name}.exe")
        } else {
            name.to_string()
        }
    }

    /// The PATH dirs a CLI may be taken from: absolute and fully expanded only. A relative
    /// entry (`.`, `bin`) would resolve against Traygent's own cwd, an unexpanded `%VAR%` is
    /// no real directory; either could plant a CLI.
    fn search_dirs(&self) -> impl Iterator<Item = &String> + '_ {
        self.path_dirs
            .iter()
            .filter(move |dir| is_search_dir(dir, self.windows))
    }

    fn in_path(&self, file: &str) -> impl Iterator<Item = String> + '_ {
        let file = file.to_string();
        self.search_dirs()
            .map(move |dir| self.join(dir, &[file.as_str()]))
    }

    /// `base` (an env var) plus the parts, or `None` when the base is not absolute: a
    /// relative `APPDATA` or `CODEX_INSTALL_DIR` would make detection run a `--version`
    /// resolved against Traygent's own cwd.
    fn under(&self, base: &Option<String>, parts: &[&str]) -> Option<String> {
        base.as_deref()
            .filter(|base| is_search_dir(base, self.windows))
            .map(|base| self.join(base, parts))
    }

    /// The host `.gitconfig` the containers mount read-only, when it exists.
    pub fn gitconfig(&self) -> Option<String> {
        self.under(&self.user_profile, &[".gitconfig"])
    }

    /// Windows PowerShell 5.1 by absolute path; the bare name only as a last resort.
    pub fn powershell(&self) -> String {
        self.under(
            &self.system_root,
            &["System32", "WindowsPowerShell", "v1.0", "powershell.exe"],
        )
        .unwrap_or_else(|| "powershell.exe".into())
    }

    /// `taskkill.exe` by absolute path; the default Windows folder when `SystemRoot` is not
    /// absolute, never the bare name (it would resolve against the cwd and PATH).
    pub fn taskkill(&self) -> String {
        self.under(&self.system_root, &["System32", "taskkill.exe"])
            .unwrap_or_else(|| r"C:\Windows\System32\taskkill.exe".into())
    }
}

/// An absolute directory without `%`: `C:\…` or UNC on Windows, `/…` elsewhere.
pub fn is_search_dir(dir: &str, windows: bool) -> bool {
    if dir.contains('%') {
        return false;
    }
    if windows {
        crate::launch::is_windows_absolute(dir)
    } else {
        dir.starts_with('/')
    }
}

/// Splits a PATH value, dropping blanks and surrounding quotes.
pub fn split_path(value: &str, windows: bool) -> Vec<String> {
    value
        .split(if windows { ';' } else { ':' })
        .map(|dir| dir.trim().trim_matches('"').to_string())
        .filter(|dir| !dir.is_empty())
        .collect()
}

pub(crate) fn dedup(dirs: Vec<String>, windows: bool) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    dirs.into_iter()
        .filter(|dir| {
            let key = dir.trim_end_matches(['\\', '/']);
            seen.insert(if windows {
                key.to_lowercase()
            } else {
                key.to_string()
            })
        })
        .collect()
}

/// Expands `%NAME%` references the way REG_EXPAND_SZ values are written; unknown names stay.
pub fn expand_vars(value: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => {
                let name = &after[..end];
                match lookup(name) {
                    Some(found) => out.push_str(&found),
                    None => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Installers write the User/Machine PATH in the registry; Traygent's own environment keeps
/// the PATH of its start until re-login (recipe §0.1), so both are read on every probe.
#[cfg(all(windows, feature = "desktop"))]
fn registry_path_dirs() -> Vec<String> {
    use winreg::{enums::*, RegKey};
    let read = |root, key: &str| {
        RegKey::predef(root)
            .open_subkey(key)
            .and_then(|key| key.get_value::<String, _>("Path"))
            .unwrap_or_default()
    };
    let lookup = |name: &str| std::env::var(name).ok();
    let user = read(HKEY_CURRENT_USER, "Environment");
    let machine = read(
        HKEY_LOCAL_MACHINE,
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
    );
    [user, machine]
        .iter()
        .flat_map(|value| split_path(&expand_vars(value, lookup), true))
        .collect()
}

#[cfg(not(all(windows, feature = "desktop")))]
fn registry_path_dirs() -> Vec<String> {
    Vec::new()
}

fn exe(path: String) -> Candidate {
    Candidate { path, shim: false }
}

fn shim(path: String) -> Candidate {
    Candidate { path, shim: true }
}

/// Every place an agent CLI may live, best first: native `.exe` files before npm `.cmd`
/// shims (recipe §0.2), PATH before the known install locations.
pub fn agent_candidates(agent: AgentId, host: &Host) -> Vec<Candidate> {
    let mut list: Vec<Candidate> = Vec::new();
    let profile = &host.user_profile;
    if !host.windows {
        let names: &[&str] = match agent {
            AgentId::Claude => &["claude"],
            AgentId::Opencode => &["opencode2", "opencode"],
            AgentId::Codex => &["codex"],
        };
        for name in names {
            list.extend(host.in_path(name).map(exe));
        }
        let known = match agent {
            AgentId::Claude => host.under(profile, &[".local", "bin", "claude"]),
            AgentId::Opencode => host.under(profile, &[".opencode", "bin", "opencode"]),
            AgentId::Codex => None,
        };
        list.extend(known.map(exe));
        return unique(list, false);
    }
    let npm = |file: &str| host.under(&host.app_data, &["npm", file]);
    match agent {
        AgentId::Claude => {
            list.extend(host.in_path(&host.exe("claude")).map(exe));
            list.extend(
                host.under(profile, &[".local", "bin", "claude.exe"])
                    .map(exe),
            );
            // WinGet's link dir (UNVERIFIED path, conventional).
            let winget = ["Microsoft", "WinGet", "Links", "claude.exe"];
            list.extend(host.under(&host.local_app_data, &winget).map(exe));
            list.extend(host.in_path("claude.cmd").map(shim));
            list.extend(npm("claude.cmd").map(shim));
        }
        AgentId::Opencode => {
            // The npm package's real binary next to its shims (layout verified on Linux only).
            let real = ["node_modules", "@opencode", "cli", "bin", "opencode.exe"];
            list.extend(host.in_path("opencode.exe").map(exe));
            list.extend(host.search_dirs().map(|dir| exe(host.join(dir, &real))));
            let npm_dir = host.under(&host.app_data, &["npm"]);
            list.extend(npm_dir.map(|dir| exe(host.join(&dir, &real))));
            let standalone = [".opencode", "bin", "opencode.exe"];
            list.extend(host.under(profile, &standalone).map(exe));
            // Only V2 installs `opencode2`; a V1 `opencode-ai` install also owns `opencode.cmd`.
            list.extend(host.in_path("opencode2.cmd").map(shim));
            list.extend(npm("opencode2.cmd").map(shim));
            list.extend(host.in_path("opencode.cmd").map(shim));
            list.extend(npm("opencode.cmd").map(shim));
        }
        AgentId::Codex => {
            list.extend(host.in_path("codex.exe").map(exe));
            let custom = &host.codex_install_dir;
            list.extend(host.under(custom, &["codex.exe"]).map(exe));
            list.extend(host.under(custom, &["bin", "codex.exe"]).map(exe));
            let native = ["Programs", "OpenAI", "Codex", "bin", "codex.exe"];
            list.extend(host.under(&host.local_app_data, &native).map(exe));
            list.extend(host.in_path("codex.cmd").map(shim));
            list.extend(npm("codex.cmd").map(shim));
        }
    }
    unique(list, true)
}

fn unique(list: Vec<Candidate>, windows: bool) -> Vec<Candidate> {
    let mut seen = std::collections::HashSet::new();
    list.into_iter()
        .filter(|candidate| {
            seen.insert(if windows {
                candidate.path.to_lowercase()
            } else {
                candidate.path.clone()
            })
        })
        .collect()
}

fn paths(list: Vec<Option<String>>, host: &Host) -> Vec<String> {
    let candidates = list.into_iter().flatten().map(exe).collect();
    unique(candidates, host.windows)
        .into_iter()
        .map(|candidate| candidate.path)
        .collect()
}

/// docker.exe: PATH, Docker Desktop, Rancher Desktop (both locations UNVERIFIED, conventional).
pub fn docker_candidates(host: &Host) -> Vec<String> {
    let mut list: Vec<Option<String>> = host.in_path(&host.exe("docker")).map(Some).collect();
    if host.windows {
        let desktop = ["Docker", "Docker", "resources", "bin", "docker.exe"];
        list.push(host.under(&host.program_files, &desktop));
        list.push(host.under(&host.user_profile, &[".rd", "bin", "docker.exe"]));
    }
    paths(list, host)
}

/// gh.exe for `gh auth token` (MSI/WinGet locations UNVERIFIED, conventional).
pub fn gh_candidates(host: &Host) -> Vec<String> {
    let mut list: Vec<Option<String>> = host.in_path(&host.exe("gh")).map(Some).collect();
    if host.windows {
        list.push(host.under(&host.program_files, &["GitHub CLI", "gh.exe"]));
        let user = ["Programs", "GitHub CLI", "gh.exe"];
        list.push(host.under(&host.local_app_data, &user));
    }
    paths(list, host)
}

/// git.exe, only to hint „Git for Windows empfohlen“ for Claude Code.
pub fn git_candidates(host: &Host) -> Vec<String> {
    let mut list: Vec<Option<String>> = host.in_path(&host.exe("git")).map(Some).collect();
    if host.windows {
        list.push(host.under(&host.program_files, &["Git", "cmd", "git.exe"]));
        let user = ["Programs", "Git", "cmd", "git.exe"];
        list.push(host.under(&host.local_app_data, &user));
    }
    paths(list, host)
}

/// wt.exe: the Store app's execution alias first, then PATH.
pub fn wt_candidates(host: &Host) -> Vec<String> {
    if !host.windows {
        return Vec::new();
    }
    let alias = ["Microsoft", "WindowsApps", "wt.exe"];
    let mut list = vec![host.under(&host.local_app_data, &alias)];
    list.extend(host.in_path("wt.exe").map(Some));
    paths(list, host)
}

/// The first candidate that exists.
pub fn first(fs: &dyn Fs, candidates: Vec<String>) -> Option<String> {
    candidates.into_iter().find(|path| fs.is_file(path))
}
