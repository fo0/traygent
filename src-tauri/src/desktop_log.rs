//! The local log from the shell (#109): the `open_log_folder` command (Settings → App
//! „Protokoll“ → „Ordner öffnen“) and the tray's „Log-Ordner öffnen“. The log itself is
//! `traygent_core::app_log`; its level is a setting, saved through `save_settings`.
use traygent_core::app_log;

const OPEN_FAILED: &str = "Log-Ordner konnte nicht geöffnet werden.";

/// Creates the log folder if it is missing and shows it in Explorer.
#[tauri::command]
pub async fn open_log_folder(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_folder(&app))
        .await
        .map_err(|_| OPEN_FAILED.to_string())?
}

/// The folder's opening for the command and the tray; a failure is logged too.
pub(crate) fn open_folder(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let opened = app_log::dir()
        .filter(|dir| std::fs::create_dir_all(dir).is_ok())
        .is_some_and(|dir| {
            app.opener()
                .open_path(dir.to_string_lossy().into_owned(), None::<&str>)
                .is_ok()
        });
    if opened {
        return Ok(());
    }
    app_log::info(&format!("log folder: {OPEN_FAILED}"));
    Err(OPEN_FAILED.to_string())
}

/// A popup that could not be shown or hidden: stderr for a development run, the log too.
pub(crate) fn popup_failed(error: &str) {
    eprintln!("Traygent popup: {error}");
    app_log::info(&format!("popup: {error}"));
}
