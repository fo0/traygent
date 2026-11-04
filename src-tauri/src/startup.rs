//! Opt-in, per-user Windows login registration. No preference copy is stored.
use serde::Serialize;
use std::{ffi::OsStr, io};

pub const AUTOSTART_ARG: &str = "--autostart";
pub const VALUE_NAME: &str = "io.github.fo0.traygent";

/// argv includes the executable, both at first launch and in SingleInstance.
pub fn show_on_launch(args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> bool {
    !args
        .into_iter()
        .skip(1)
        .any(|arg| arg.as_ref() == OsStr::new(AUTOSTART_ARG))
}

pub fn registration_command(executable: &str) -> Result<String, String> {
    if executable.is_empty() || executable.contains(['"', '\0', '\r', '\n']) {
        return Err("Der Installationspfad ist für Windows-Autostart ungeeignet.".into());
    }
    let command = format!("\"{executable}\" {AUTOSTART_ARG}");
    // Microsoft documents a 260-character limit for a Run command line.
    if command.encode_utf16().count() >= 260 {
        return Err("Der Installationspfad ist für Windows-Autostart zu lang.".into());
    }
    Ok(command)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Unsupported,
    Disabled,
    Enabled,
    DisabledByWindows,
    UnknownApproval,
    Conflict,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupStatus {
    pub status: Status,
    pub registered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    Allowed,
    Blocked,
    Unknown,
}

/// StartupApproved is Windows-owned. Read known formats conservatively, never
/// write/delete them or treat a future/invalid format as enabled.
pub fn approval_from_bytes(bytes: &[u8]) -> Approval {
    if bytes.len() != 12 {
        return Approval::Unknown;
    }
    match u32::from_le_bytes(bytes[..4].try_into().expect("four bytes")) {
        2 | 6 if bytes[4..].iter().all(|byte| *byte == 0) => Approval::Allowed,
        3 | 7 => Approval::Blocked,
        _ => Approval::Unknown,
    }
}

pub trait Registration {
    fn command(&self) -> io::Result<Option<String>>;
    fn approval(&self) -> io::Result<Approval>;
    /// Only this application's dedicated value; never a whole key.
    fn write(&mut self, command: Option<&str>) -> io::Result<()>;
}

pub fn read(store: &impl Registration, expected: &str) -> Result<StartupStatus, String> {
    let command = store.command().map_err(|_| read_error())?;
    let status = match command.as_deref() {
        None => Status::Disabled,
        Some(command) if command != expected => Status::Conflict,
        Some(_) => match store.approval().map_err(|_| read_error())? {
            Approval::Allowed => Status::Enabled,
            Approval::Blocked => Status::DisabledByWindows,
            Approval::Unknown => Status::UnknownApproval,
        },
    };
    Ok(StartupStatus {
        status,
        registered: command.as_deref() == Some(expected),
    })
}

fn read_error() -> String {
    "Windows-Autostart konnte nicht gelesen werden. Prüfe den Status erneut.".into()
}

pub fn set_enabled(
    store: &mut impl Registration,
    expected: &str,
    enabled: bool,
) -> Result<StartupStatus, String> {
    let current = store.command().map_err(|_| read_error())?;
    if current.as_deref().is_some_and(|value| value != expected) {
        return Err("Der Autostartname wird von einem anderen Eintrag verwendet. Es wurde nichts verändert.".into());
    }
    if enabled != current.is_some() {
        store
            .write(enabled.then_some(expected))
            .map_err(|_| "Windows-Autostart konnte nicht geändert werden. Prüfe den Status und versuche es erneut.".to_string())?;
    }
    let actual = read(store, expected)?;
    if actual.registered != enabled {
        return Err(
            "Windows hat die Änderung nicht bestätigt. Prüfe den Autostartstatus erneut.".into(),
        );
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Default)]
    struct Mock {
        command: Option<String>,
        blocked: bool,
        read_fails: bool,
        write_fails: bool,
        ignore_write: bool,
        approval_fails: bool,
        writes: usize,
        reads: Cell<usize>,
    }
    impl Registration for Mock {
        fn command(&self) -> io::Result<Option<String>> {
            self.reads.set(self.reads.get() + 1);
            if self.read_fails {
                Err(io::ErrorKind::PermissionDenied.into())
            } else {
                Ok(self.command.clone())
            }
        }
        fn approval(&self) -> io::Result<Approval> {
            if self.approval_fails {
                Err(io::ErrorKind::PermissionDenied.into())
            } else if self.blocked {
                Ok(Approval::Blocked)
            } else {
                Ok(Approval::Allowed)
            }
        }
        fn write(&mut self, command: Option<&str>) -> io::Result<()> {
            self.writes += 1;
            if self.write_fails {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            if !self.ignore_write {
                self.command = command.map(str::to_owned);
            }
            Ok(())
        }
    }
    fn expected() -> String {
        registration_command(r"C:\Users\Test Person\Traygent\traygent.exe").unwrap()
    }

    #[test]
    fn manual_and_duplicate_manual_launch_show_but_login_launches_do_not() {
        assert!(show_on_launch(["traygent.exe"]));
        assert!(show_on_launch(["traygent.exe", "--other"]));
        assert!(!show_on_launch(["traygent.exe", AUTOSTART_ARG]));
        assert!(!show_on_launch(["traygent.exe", "--other", AUTOSTART_ARG]));
        assert!(show_on_launch([AUTOSTART_ARG])); // executable is not a flag
        assert!(show_on_launch(["traygent.exe", "--autostart=false"]));
    }

    #[test]
    fn quoted_unicode_path_and_run_length_limit() {
        assert_eq!(
            registration_command(r"C:\Täst User\traygent.exe").unwrap(),
            "\"C:\\Täst User\\traygent.exe\" --autostart"
        );
        for invalid in ["", "bad\"path", "bad\0path", "bad\npath"] {
            assert!(registration_command(invalid).is_err());
        }
        assert!(registration_command(&"x".repeat(245)).is_ok());
        assert!(registration_command(&"x".repeat(246)).is_err());
        assert!(registration_command(&"😀".repeat(124)).is_err());
    }

    #[test]
    fn default_reads_and_restarts_never_enable_or_write() {
        let store = Mock::default();
        for _ in 0..3 {
            assert_eq!(read(&store, &expected()).unwrap().status, Status::Disabled);
        }
        assert_eq!(store.writes, 0);
    }

    #[test]
    fn enable_disable_idempotence_and_external_changes_are_read_back() {
        let mut store = Mock::default();
        assert_eq!(
            set_enabled(&mut store, &expected(), true).unwrap().status,
            Status::Enabled
        );
        assert_eq!(store.command.as_deref(), Some(expected().as_str()));
        set_enabled(&mut store, &expected(), true).unwrap();
        assert_eq!(store.writes, 1);
        store.blocked = true;
        assert_eq!(
            read(&store, &expected()).unwrap().status,
            Status::DisabledByWindows
        );
        // Explicit disable still removes our entry when Windows blocks approval reads.
        store.approval_fails = true;
        assert_eq!(
            set_enabled(&mut store, &expected(), false).unwrap().status,
            Status::Disabled
        );
        set_enabled(&mut store, &expected(), false).unwrap();
        assert_eq!(store.writes, 2);
        assert_eq!(store.command, None);
    }

    #[test]
    fn windows_veto_is_preserved_even_on_explicit_enable() {
        let mut store = Mock {
            blocked: true,
            ..Mock::default()
        };
        let value = set_enabled(&mut store, &expected(), true).unwrap();
        assert_eq!(value.status, Status::DisabledByWindows);
        assert!(value.registered);
        assert!(store.blocked);
    }

    #[test]
    fn foreign_or_relocated_installations_are_never_overwritten_or_removed() {
        for command in ["other.exe", "\"C:\\Old\\traygent.exe\" --autostart"] {
            let mut store = Mock {
                command: Some(command.into()),
                ..Mock::default()
            };
            assert_eq!(read(&store, &expected()).unwrap().status, Status::Conflict);
            assert!(set_enabled(&mut store, &expected(), true).is_err());
            assert!(set_enabled(&mut store, &expected(), false).is_err());
            assert_eq!(store.command.as_deref(), Some(command));
            assert_eq!(store.writes, 0);
        }
    }

    #[test]
    fn failed_reads_writes_and_unconfirmed_changes_never_report_success() {
        let mut store = Mock {
            read_fails: true,
            ..Mock::default()
        };
        assert!(read(&store, &expected()).is_err());
        assert!(set_enabled(&mut store, &expected(), true).is_err());
        assert_eq!(store.writes, 0);
        store.read_fails = false;
        store.write_fails = true;
        assert!(set_enabled(&mut store, &expected(), true).is_err());
        assert_eq!(store.command, None);
        store.write_fails = false;
        store.ignore_write = true;
        assert!(set_enabled(&mut store, &expected(), true).is_err());
    }

    #[test]
    fn known_windows_approval_and_future_formats() {
        for state in [2_u32, 6] {
            let mut value = [0; 12];
            value[..4].copy_from_slice(&state.to_le_bytes());
            assert_eq!(approval_from_bytes(&value), Approval::Allowed);
        }
        for state in [3_u32, 7] {
            let mut value = [1; 12];
            value[..4].copy_from_slice(&state.to_le_bytes());
            assert_eq!(approval_from_bytes(&value), Approval::Blocked);
        }
        assert_eq!(approval_from_bytes(&[0; 12]), Approval::Unknown);
        assert_eq!(approval_from_bytes(&[2]), Approval::Unknown);
    }
}
