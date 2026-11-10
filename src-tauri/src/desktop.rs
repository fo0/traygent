//! The desktop shell: tray icon, popup window, IPC commands and the plugins they use.
use std::sync::{Mutex, MutexGuard, PoisonError};
use tauri::{menu::Menu, menu::MenuItem, tray::TrayIconBuilder, Manager};
use traygent_core::service::Service;
use traygent_core::Snapshot;

/// The native service state: settings, recent sessions, detection results and the GitHub
/// account. Commands reach it only off the UI thread, through `work` or a short lock that is
/// never held across network or process calls.
pub(crate) struct NativeState {
    pub(crate) service: Mutex<Service>,
}

impl NativeState {
    /// A panic in an earlier operation poisons the lock; every `Service` change is committed
    /// only after it succeeded, so the state behind it is still whole and stays usable.
    pub(crate) fn lock(&self) -> MutexGuard<'_, Service> {
        self.service.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A worker thread that panicked or was cancelled.
pub(crate) const FAILED: &str = "Aufgabe konnte nicht abgeschlossen werden.";

/// Runs `operation` on a blocking worker thread with exclusive access to the native state.
pub(crate) async fn work<T: Send + 'static>(
    app: tauri::AppHandle,
    operation: impl FnOnce(&mut Service) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<NativeState>();
        let mut service = state.lock();
        operation(&mut service)
    })
    .await
    .map_err(|_| FAILED.to_string())?
}

#[tauri::command]
async fn snapshot(app: tauri::AppHandle) -> Result<Snapshot, String> {
    work(app, |service| Ok(service.snapshot())).await
}

/// Returns at once with `detecting: true`; the finished snapshot follows as an event.
#[tauri::command]
async fn refresh_detection(app: tauri::AppHandle) -> Result<Snapshot, String> {
    let handle = app.clone();
    work(app, move |service| {
        crate::desktop_agents::start_detection(&handle, service);
        Ok(service.snapshot())
    })
    .await
}

#[tauri::command]
fn hide_panel(app: tauri::AppHandle) -> Result<(), String> {
    crate::tray_popup::hide(&app)
}

pub fn run() {
    let show_panel = traygent_core::startup::show_on_launch(std::env::args_os());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            // A duplicate login launch must neither show nor hide the existing panel.
            if traygent_core::startup::show_on_launch(args) {
                if let Err(error) = crate::tray_popup::show(app) {
                    eprintln!("Traygent popup: {error}");
                }
            }
        }))
        .manage(crate::desktop_startup::StartupState::default())
        .manage(crate::tray_popup::Popup::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(crate::desktop_updates::UpdateState::default())
        .setup(move |app| {
            let mut service = match app.path().app_data_dir() {
                Ok(dir) => Service::open(dir),
                Err(_) => Service::read_only(traygent_core::store::NO_APP_DATA),
            };
            let host = traygent_core::detect::Host::current();
            service.set_default_clone_root(traygent_core::repo::default_clone_root(&host));
            app.manage(NativeState {
                service: Mutex::new(service),
            });
            let state = app.state::<NativeState>();
            let mut service = state.lock();
            crate::desktop_agents::start_detection(app.handle(), &mut service);
            // A loaded account lists its repositories at startup.
            crate::desktop_github::start_listing(app.handle(), &mut service);
            drop(service);
            crate::desktop_agent_updates::start_schedule(app.handle());
            let show = MenuItem::with_id(
                app,
                "toggle",
                "Traygent öffnen / ausblenden",
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "Traygent beenden", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::with_id("traygent")
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/32x32.png"
                ))?)
                .tooltip("Traygent")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "toggle" => crate::tray_popup::toggle(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    crate::tray_popup::tray_event(tray.app_handle(), event);
                })
                .build(app)?;
            // Keep visible:false throughout a login launch; there is no show/hide flash.
            if show_panel {
                crate::tray_popup::show(app.handle()).map_err(std::io::Error::other)?;
            }
            Ok(())
        })
        .on_window_event(crate::tray_popup::window_event)
        .invoke_handler(tauri::generate_handler![
            snapshot,
            refresh_detection,
            crate::desktop_agents::save_settings,
            crate::desktop_agents::launch_session,
            crate::desktop_agents::remove_recent,
            crate::desktop_agents::restore_recent,
            crate::desktop_agents::update_recent,
            crate::desktop_agents::pick_folder,
            crate::desktop_agents::install_agent,
            crate::desktop_agents::open_folder,
            crate::desktop_agent_updates::update_agents,
            crate::desktop_github::github_connect_token,
            crate::desktop_github::github_local_accounts,
            crate::desktop_github::github_import_local,
            crate::desktop_github::github_disconnect,
            crate::desktop_github::github_refresh_repos,
            crate::desktop_startup::autostart_status,
            crate::desktop_startup::set_autostart,
            crate::desktop_startup::open_startup_settings,
            crate::desktop_updates::check_update,
            crate::desktop_updates::install_update,
            hide_panel
        ])
        .run(tauri::generate_context!())
        .expect("Traygent konnte nicht gestartet werden");
}
