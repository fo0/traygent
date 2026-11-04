//! Windows' per-user Run value, scoped to the installed executable. We use the
//! documented Run mechanism directly so ownership checks precede every mutation
//! and no plugin can fall back to HKLM or override Windows' startup veto.
use std::sync::Mutex;
use traygent_core::startup::StartupStatus;
#[cfg(not(windows))]
use traygent_core::startup::Status;

#[derive(Default)]
pub struct StartupState(Mutex<()>);

#[tauri::command]
pub fn autostart_status(state: tauri::State<'_, StartupState>) -> Result<StartupStatus, String> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Autostartdienst nicht verfügbar.")?;
    status()
}

#[tauri::command]
pub fn set_autostart(
    state: tauri::State<'_, StartupState>,
    enabled: bool,
) -> Result<StartupStatus, String> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Autostartdienst nicht verfügbar.")?;
    #[cfg(windows)]
    {
        let expected = windows::expected()?;
        traygent_core::startup::set_enabled(
            &mut windows::Registry::current_user(),
            &expected,
            enabled,
        )
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        status()
    }
}

fn status() -> Result<StartupStatus, String> {
    #[cfg(windows)]
    {
        traygent_core::startup::read(&windows::Registry::current_user(), &windows::expected()?)
    }
    #[cfg(not(windows))]
    Ok(StartupStatus {
        status: Status::Unsupported,
        registered: false,
    })
}

#[tauri::command]
pub fn open_startup_settings(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener().open_url("ms-settings:startupapps", None::<&str>)
            .map_err(|_| "Windows-Autostart konnte nicht geöffnet werden. Öffne Windows-Einstellungen → Apps → Autostart.".into())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("Windows-Autostart ist nur unter Windows verfügbar.".into())
    }
}

#[cfg(windows)]
mod windows {
    use std::{io, path::Path};
    use traygent_core::startup::{self, Approval, Registration, VALUE_NAME};
    use winreg::{enums::*, RegKey};

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const APPROVAL: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    pub(super) fn expected() -> Result<String, String> {
        let executable = std::env::current_exe()
            .map_err(|_| "Installationspfad konnte nicht gelesen werden.")?;
        command_for_path(&executable)
    }

    fn command_for_path(path: &Path) -> Result<String, String> {
        if !path.is_absolute() {
            return Err("Windows-Autostart benötigt einen absoluten Installationspfad.".into());
        }
        startup::registration_command(
            path.to_str()
                .ok_or("Installationspfad konnte nicht gelesen werden.")?,
        )
    }

    pub(super) struct Registry {
        root: RegKey,
    }

    impl Registry {
        pub(super) fn current_user() -> Self {
            Self {
                root: RegKey::predef(HKEY_CURRENT_USER),
            }
        }

        fn optional<T>(result: io::Result<T>) -> io::Result<Option<T>> {
            match result {
                Ok(value) => Ok(Some(value)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error),
            }
        }
    }

    impl Registration for Registry {
        fn command(&self) -> io::Result<Option<String>> {
            let Some(key) = Self::optional(self.root.open_subkey_with_flags(RUN, KEY_QUERY_VALUE))?
            else {
                return Ok(None);
            };
            Self::optional(key.get_value(VALUE_NAME))
        }

        fn approval(&self) -> io::Result<Approval> {
            let Some(key) =
                Self::optional(self.root.open_subkey_with_flags(APPROVAL, KEY_QUERY_VALUE))?
            else {
                return Ok(Approval::Allowed);
            };
            let Some(value) = Self::optional(key.get_raw_value(VALUE_NAME))? else {
                return Ok(Approval::Allowed);
            };
            Ok(if value.vtype == REG_BINARY {
                startup::approval_from_bytes(&value.bytes)
            } else {
                Approval::Unknown
            })
        }

        fn write(&mut self, command: Option<&str>) -> io::Result<()> {
            if let Some(command) = command {
                let (key, _) = self.root.create_subkey_with_flags(RUN, KEY_SET_VALUE)?;
                key.set_value(VALUE_NAME, &command)
            } else {
                // Missing is already disabled. Never create a key just to remove a value.
                if let Some(key) =
                    Self::optional(self.root.open_subkey_with_flags(RUN, KEY_SET_VALUE))?
                {
                    Self::optional(key.delete_value(VALUE_NAME))?;
                }
                Ok(())
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::time::{SystemTime, UNIX_EPOCH};
        use winreg::RegValue;

        #[test]
        fn registry_roundtrip_preserves_neighbors_and_windows_override() {
            // An isolated test root: never register an actual startup command in CI.
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let path = format!(
                r"Software\io.github.fo0.traygent\Tests\{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            let (root, _) = hkcu.create_subkey(&path).unwrap();
            let mut registry = Registry { root };
            let (run, _) = registry.root.create_subkey(RUN).unwrap();
            run.set_value("unrelated", &"leave me").unwrap();
            let expected =
                command_for_path(Path::new(r"C:\Users\Test Person\Traygent\traygent.exe")).unwrap();
            assert_eq!(
                startup::read(&registry, &expected).unwrap().status,
                startup::Status::Disabled
            );
            startup::set_enabled(&mut registry, &expected, true).unwrap();
            assert_eq!(run.get_value::<String, _>(VALUE_NAME).unwrap(), expected);
            let (approval, _) = registry.root.create_subkey(APPROVAL).unwrap();
            let veto = RegValue {
                vtype: REG_BINARY,
                bytes: vec![3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0].into(),
            };
            approval.set_raw_value(VALUE_NAME, &veto).unwrap();
            assert_eq!(
                startup::read(&registry, &expected).unwrap().status,
                startup::Status::DisabledByWindows
            );
            startup::set_enabled(&mut registry, &expected, false).unwrap();
            assert!(registry.command().unwrap().is_none());
            assert_eq!(run.get_value::<String, _>("unrelated").unwrap(), "leave me");
            assert_eq!(
                approval.get_raw_value(VALUE_NAME).unwrap().bytes,
                veto.bytes
            );
            run.set_value(VALUE_NAME, &"foreign.exe").unwrap();
            assert!(startup::set_enabled(&mut registry, &expected, false).is_err());
            assert_eq!(
                run.get_value::<String, _>(VALUE_NAME).unwrap(),
                "foreign.exe"
            );
            drop(approval);
            drop(run);
            drop(registry);
            hkcu.delete_subkey_all(path).unwrap();
        }
    }
}
