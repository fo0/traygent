//! Test support shared by the unit tests: a fake file system, a Windows host, temp dirs.
use crate::detect::{Fs, Host};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

pub(crate) const NPM: &str = r"C:\Users\max\AppData\Roaming\npm";

/// Answers from fixed sets of files and directories; records the directories it is asked
/// to create (and refuses to when `read_only`).
#[derive(Default)]
pub(crate) struct FakeFs {
    pub files: HashSet<String>,
    pub dirs: HashSet<String>,
    pub created: Mutex<Vec<String>>,
    pub read_only: bool,
    /// Profile directories that hold someone else's data.
    pub foreign: HashSet<String>,
}

impl FakeFs {
    pub(crate) fn with(files: &[&str]) -> Self {
        Self {
            files: files.iter().map(|file| file.to_string()).collect(),
            ..Self::default()
        }
    }

    pub(crate) fn created(&self) -> Vec<String> {
        self.created.lock().unwrap().clone()
    }

    pub(crate) fn dir(mut self, dir: &str) -> Self {
        self.dirs.insert(dir.into());
        self
    }
}

impl Fs for FakeFs {
    fn is_file(&self, path: &str) -> bool {
        self.files.contains(path)
    }
    fn is_dir(&self, path: &str) -> bool {
        self.dirs.contains(path)
    }
    fn create_dir_all(&self, path: &str) -> std::io::Result<()> {
        if self.read_only {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        self.created.lock().unwrap().push(path.into());
        Ok(())
    }
    fn refuses_sign_in(
        &self,
        _: crate::models::AgentId,
        dir: &str,
        _: &str,
        _: &crate::models::ProfileShare,
    ) -> bool {
        self.foreign.contains(dir)
    }
}

/// A Windows user `max` with System32 and the npm dir on PATH.
pub(crate) fn windows_host() -> Host {
    Host {
        windows: true,
        path_dirs: vec![r"C:\Windows\system32".into(), NPM.into()],
        user_profile: Some(r"C:\Users\max".into()),
        app_data: Some(r"C:\Users\max\AppData\Roaming".into()),
        local_app_data: Some(r"C:\Users\max\AppData\Local".into()),
        program_files: Some(r"C:\Program Files".into()),
        system_root: Some(r"C:\Windows".into()),
        codex_install_dir: None,
    }
}

/// A fresh directory under the system temp dir, removed when dropped.
pub(crate) struct TempDir(pub PathBuf);

impl TempDir {
    pub(crate) fn new(name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "traygent-test-{name}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub(crate) fn entries(&self) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Directory links for the engine tests on every platform: symbolic links, so no test
/// spawns PowerShell. The real junction linker has one dedicated Windows test.
pub(crate) struct TestLinker;

impl crate::profiles::Linker for TestLinker {
    fn link(&self, links: &[crate::profiles::links::Link]) {
        for pair in links {
            #[cfg(unix)]
            let _ = std::os::unix::fs::symlink(&pair.target, &pair.link);
            #[cfg(windows)]
            let _ = std::os::windows::fs::symlink_dir(&pair.target, &pair.link);
        }
    }
}

/// A linker that creates nothing: PowerShell blocked, a timeout.
pub(crate) struct NoLinker;

impl crate::profiles::Linker for NoLinker {
    fn link(&self, _: &[crate::profiles::links::Link]) {}
}
