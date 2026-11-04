//! Finds the gh.exe a token may be read from: the detection's candidates (PATH incl. the
//! registry PATH, Program Files, LocalAppData) plus RepoWatch's further install locations
//! (64/32-bit Program Files, WinGet links, Scoop, Chocolatey). A candidate must be an
//! absolute, existing `.exe` outside the working directory and outside any Git checkout;
//! it is canonicalized before it runs.
use crate::detect::host::{gh_candidates, is_search_dir, Host};
use std::path::{Path, PathBuf};

/// Every place gh.exe may live, best first. Windows rules; empty on other hosts.
pub fn gh_paths(host: &Host, var: impl Fn(&str) -> Option<String>) -> Vec<String> {
    if !host.windows {
        return Vec::new();
    }
    let root = |name: &str| var(name).filter(|value| is_search_dir(value, true));
    let mut list = gh_candidates(host);
    for name in ["ProgramW6432", "ProgramFiles(x86)"] {
        list.extend(root(name).map(|dir| host.join(&dir, &["GitHub CLI", "gh.exe"])));
    }
    let winget = ["Microsoft", "WinGet", "Links", "gh.exe"];
    list.extend(
        host.local_app_data
            .as_deref()
            .filter(|dir| is_search_dir(dir, true))
            .map(|dir| host.join(dir, &winget)),
    );
    let scoop = ["apps", "gh", "current", "bin", "gh.exe"];
    let profile = host
        .user_profile
        .as_deref()
        .filter(|dir| is_search_dir(dir, true))
        .map(|dir| host.join(dir, &["scoop"]));
    let data = root("ProgramData").map(|dir| host.join(&dir, &["scoop"]));
    let scoop_roots = [profile, data, root("SCOOP"), root("SCOOP_GLOBAL")];
    list.extend(
        scoop_roots
            .into_iter()
            .flatten()
            .map(|dir| host.join(&dir, &scoop)),
    );
    if let Some(choco) = root("ChocolateyInstall") {
        list.push(host.join(&choco, &["bin", "gh.exe"]));
        list.push(host.join(&choco, &["lib", "gh", "tools", "bin", "gh.exe"]));
    }
    let mut seen = std::collections::HashSet::new();
    list.retain(|path| seen.insert(path.to_lowercase()));
    list
}

/// The canonical path of `path` when it may run: absolute, an existing `.exe` file (also
/// after resolving links), not in `cwd`, not inside a Git checkout (a project-local binary).
pub fn safe_executable(path: &Path, cwd: &Path) -> Option<PathBuf> {
    let exe = |path: &Path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    };
    if !path.is_absolute() || !path.is_file() || !exe(path) {
        return None;
    }
    let real = path.canonicalize().ok()?;
    // A `gh.exe` link to a `.cmd`/`.bat` would run through cmd.exe: the target counts.
    if !exe(&real) {
        return None;
    }
    let working = cwd.canonicalize().ok();
    let parent = path.parent().and_then(|parent| parent.canonicalize().ok());
    if (parent.is_some() && parent == working)
        || (working.is_some() && real.parent() == working.as_deref())
        || real.ancestors().any(|dir| {
            let git = dir.join(".git");
            git.is_file() || git.join("HEAD").is_file()
        })
    {
        return None;
    }
    Some(real)
}

/// The first safe gh.exe of [`gh_paths`].
pub fn find_gh(host: &Host, var: impl Fn(&str) -> Option<String>, cwd: &Path) -> Option<PathBuf> {
    gh_paths(host, var)
        .into_iter()
        .find_map(|path| safe_executable(Path::new(&path), cwd))
}

/// gh.exe on this PC; `None` on other platforms (Traygent imports only on Windows).
pub fn installed_gh(host: &Host) -> Option<PathBuf> {
    let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    let cwd = std::env::current_dir().unwrap_or_default();
    find_gh(host, var, &cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{windows_host, TempDir};
    use std::collections::BTreeMap;

    #[test]
    fn windows_candidates_cover_installers_package_managers_and_path() {
        let mut host = windows_host();
        host.path_dirs.push(r"C:\tools".into());
        host.path_dirs.push(r"relative\bin".into());
        let env: BTreeMap<&str, &str> = [
            ("ProgramW6432", r"C:\Program Files"),
            ("ProgramFiles(x86)", r"C:\Program Files (x86)"),
            ("ProgramData", r"C:\ProgramData"),
            ("SCOOP", r"D:\scoop"),
            ("SCOOP_GLOBAL", r"relative\scoop"),
            ("ChocolateyInstall", r"C:\ProgramData\chocolatey"),
        ]
        .into_iter()
        .collect();
        let paths = gh_paths(&host, |name| env.get(name).map(|v| v.to_string()));
        let expected = [
            r"C:\Windows\system32\gh.exe",
            r"C:\Users\max\AppData\Roaming\npm\gh.exe",
            r"C:\tools\gh.exe",
            r"C:\Program Files\GitHub CLI\gh.exe",
            r"C:\Users\max\AppData\Local\Programs\GitHub CLI\gh.exe",
            r"C:\Program Files (x86)\GitHub CLI\gh.exe",
            r"C:\Users\max\AppData\Local\Microsoft\WinGet\Links\gh.exe",
            r"C:\Users\max\scoop\apps\gh\current\bin\gh.exe",
            r"C:\ProgramData\scoop\apps\gh\current\bin\gh.exe",
            r"D:\scoop\apps\gh\current\bin\gh.exe",
            r"C:\ProgramData\chocolatey\bin\gh.exe",
            r"C:\ProgramData\chocolatey\lib\gh\tools\bin\gh.exe",
        ];
        assert_eq!(paths.len(), expected.len(), "{paths:#?}");
        for path in expected {
            assert!(paths.iter().any(|p| p == path), "{path}");
        }
        // PATH first, relative entries and roots never.
        assert_eq!(paths[2], r"C:\tools\gh.exe");
        assert!(!paths.iter().any(|p| p.contains("relative")));
        let mut linux = host.clone();
        linux.windows = false;
        assert!(gh_paths(&linux, |_| None).is_empty());
    }

    #[test]
    fn project_binaries_cwd_binaries_and_non_exe_files_are_rejected() {
        let root = TempDir::new("gh-discovery");
        let installed = |suffix: &str| {
            let path = root.0.join(suffix);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"synthetic executable; never run").unwrap();
            path
        };
        let cwd = root.0.join("cwd");
        std::fs::create_dir_all(&cwd).unwrap();
        let custom = installed("custom/bin/gh.exe");
        let project = installed("project/bin/gh.exe");
        std::fs::create_dir_all(root.0.join("project/.git")).unwrap();
        std::fs::write(root.0.join("project/.git/HEAD"), "ref: refs/heads/main").unwrap();
        let worktree = installed("worktree/bin/gh.exe");
        std::fs::write(root.0.join("worktree/.git"), "gitdir: elsewhere").unwrap();
        let in_cwd = installed("cwd/gh.exe");
        let script = installed("custom/bin/gh.cmd");
        assert_eq!(
            safe_executable(&custom, &cwd),
            Some(custom.canonicalize().unwrap())
        );
        for rejected in [&project, &worktree, &in_cwd, &script] {
            assert!(safe_executable(rejected, &cwd).is_none(), "{rejected:?}");
        }
        assert!(safe_executable(Path::new("gh.exe"), &cwd).is_none());
        assert!(safe_executable(&root.0.join("missing/gh.exe"), &cwd).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn an_exe_link_to_a_script_is_rejected() {
        let root = TempDir::new("gh-discovery-link");
        let cwd = root.0.join("cwd");
        let bin = root.0.join("bin");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        let script = bin.join("gh.cmd");
        std::fs::write(&script, b"synthetic script; never run").unwrap();
        let link = bin.join("gh.exe");
        std::os::unix::fs::symlink(&script, &link).unwrap();
        assert!(safe_executable(&link, &cwd).is_none());
        let real = bin.join("real.exe");
        std::fs::write(&real, b"synthetic executable; never run").unwrap();
        let alias = bin.join("alias.exe");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        assert_eq!(
            safe_executable(&alias, &cwd),
            Some(real.canonicalize().unwrap())
        );
    }
}
