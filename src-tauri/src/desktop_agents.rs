//! The agent-domain IPC commands: settings, detection, launching, recent sessions, folders.
//! Names and arguments are the contract with `src/lib/ipc.ts` (docs/SPEC.md → IPC contract).
use crate::desktop::{work, NativeState};
use tauri::{AppHandle, Emitter, Manager};
use traygent_core::detect::{self, Host, RealFs, SystemRunner};
use traygent_core::launch::{self, LaunchPlan};
use traygent_core::service::{self, DetectionEnd, Service};
use traygent_core::{
    spawn, AgentId, AppSettings, LaunchOptions, LaunchOutcome, LaunchRequest, PerAgent,
    RecentSession,
};

/// Panel event carrying the snapshot of a finished detection.
pub const SNAPSHOT_EVENT: &str = "traygent:snapshot";
const FOLDER_MISSING: &str = "Ordner nicht gefunden.";

/// Starts a detection on its own thread unless one runs; the finished snapshot is emitted.
pub(crate) fn start_detection(app: &AppHandle, service: &mut Service) {
    if service.begin_detection() {
        run_detection(app, service);
    }
}

/// Runs the detection `service` already marked as begun. `detecting` is cleared on every
/// path: a panicking probe ends as a failed run, a thread that cannot start aborts it.
pub(crate) fn run_detection(app: &AppHandle, service: &mut Service) {
    let images = service.settings().docker.images.clone();
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("traygent-detection".into())
        .spawn(move || detection_thread(&app, images));
    if spawned.is_err() {
        service.abort_detection();
    }
}

fn detection_thread(app: &AppHandle, mut images: PerAgent<String>) {
    loop {
        let detection = std::panic::catch_unwind(|| {
            let runner = SystemRunner::default();
            detect::detect(
                &Host::current(),
                &RealFs,
                &runner,
                &images,
                chrono::Utc::now(),
            )
        })
        .ok();
        let state = app.state::<NativeState>();
        crate::desktop_profiles::read_or_sync(&state);
        let mut service = state.lock();
        match service.finish_detection(detection) {
            DetectionEnd::Finished(snapshot) => {
                drop(service);
                if let Err(error) = app.emit(SNAPSHOT_EVENT, &snapshot) {
                    eprintln!("Traygent: {error}");
                }
                return;
            }
            // Settings changed the images meanwhile: probe again, still `detecting`.
            DetectionEnd::Rerun => images = service.settings().docker.images.clone(),
        }
    }
}

/// Spawns the plan; a failed wt.exe start falls back to a console window.
pub(crate) fn start(plan: &LaunchPlan, fallback: Option<&LaunchPlan>) -> Result<(), String> {
    match (spawn::spawn(plan), fallback) {
        (Ok(()), _) => Ok(()),
        (Err(_), Some(console)) => spawn::spawn(console),
        (Err(error), None) => Err(error),
    }
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    settings: AppSettings,
) -> Result<traygent_core::Snapshot, String> {
    let handle = app.clone();
    let (plan, changed) = work(app.clone(), move |service| {
        let saved = service.save_settings(settings)?;
        if saved.start_detection {
            run_detection(&handle, service);
        }
        Ok((service.profile_plan(), saved.profiles_changed))
    })
    .await?;
    // A changed profile list is synced at once; the statuses are read anew either way.
    // Without the service lock: the settings are saved, whatever the sync does.
    let outcome = crate::desktop_profiles::run_plan(plan, changed).await;
    work(app, move |service| {
        if let Ok(outcome) = outcome {
            service.apply_profiles(outcome);
        }
        Ok(service.snapshot())
    })
    .await
}

#[tauri::command]
pub async fn launch_session(
    app: AppHandle,
    request: LaunchRequest,
) -> Result<LaunchOutcome, String> {
    let (prepared, plan) = work(app.clone(), move |service| {
        let host = Host::current();
        let prepared = service.prepare_launch(&request, &host, &RealFs, &service::nonce())?;
        let plan = service.launch_sync_plan(&prepared.request);
        Ok((prepared, plan))
    })
    .await?;
    // Every profile of the agent is brought in line before its session starts — without
    // the service lock, so the panel stays responsive during a long first adoption.
    // A sync that broke down (its worker panicked) is a warning like any other sync
    // problem: the session still starts.
    let synced = match plan {
        Some(plan) => Some(crate::desktop_profiles::run_plan(plan, true).await),
        None => None,
    };
    let outcome = work(app.clone(), move |service| {
        // The lock was free meanwhile: the agent may be updating now, the profile gone.
        service.confirm_launch(&prepared.request)?;
        let warning = match synced {
            Some(Ok(synced)) => service.finish_launch_sync(&prepared.request, synced)?,
            Some(Err(_)) => Some(service::SYNC_WARNING.to_string()),
            None => None,
        };
        start(&prepared.plan, prepared.fallback.as_ref())?;
        let recent = service.record_launch(&prepared.request, chrono::Utc::now());
        Ok(LaunchOutcome {
            recent,
            snapshot: service.snapshot(),
            warning,
        })
    })
    .await?;
    // The new terminal should get the focus, not the popup.
    if let Err(error) = crate::tray_popup::hide(&app) {
        eprintln!("Traygent popup: {error}");
    }
    Ok(outcome)
}

#[tauri::command]
pub async fn remove_recent(app: AppHandle, id: String) -> Result<traygent_core::Snapshot, String> {
    work(app, move |service| service.remove_recent(&id)).await
}

#[tauri::command]
pub async fn restore_recent(
    app: AppHandle,
    recent: RecentSession,
) -> Result<traygent_core::Snapshot, String> {
    work(app, move |service| service.restore_recent(recent)).await
}

#[tauri::command]
pub async fn update_recent(
    app: AppHandle,
    id: String,
    options: LaunchOptions,
    ports: Vec<String>,
) -> Result<traygent_core::Snapshot, String> {
    work(app, move |service| {
        service.update_recent(&id, options, ports)
    })
    .await
}

/// The native folder dialog, owned by the panel so the popup stays open while it shows.
#[tauri::command]
pub async fn pick_folder(
    app: AppHandle,
    initial: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = app.dialog().file().set_title("Ordner wählen");
        // Only a folder `normalize_folder` accepts (no device, stream or reserved names).
        let initial = initial.and_then(|dir| launch::normalize_folder(&dir).ok());
        if let Some(dir) = initial.filter(|dir| std::path::Path::new(dir).is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(window) = app.get_webview_window("main") {
            dialog = dialog.set_parent(&window);
        }
        dialog
            .blocking_pick_folder()
            .map(|picked| {
                picked
                    .simplified()
                    .into_path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .map_err(|_| "Der gewählte Ordner kann nicht verwendet werden.".to_string())
            })
            .transpose()
    })
    .await
    .map_err(|_| "Ordnerauswahl fehlgeschlagen.".to_string())?
}

/// Opens a terminal running the catalog install command; only ever on an explicit click.
#[tauri::command]
pub async fn install_agent(app: AppHandle, agent: AgentId) -> Result<(), String> {
    work(app, move |service| {
        let (plan, fallback) = service.prepare_install(agent, &Host::current(), &RealFs)?;
        start(&plan, fallback.as_ref())
    })
    .await
}

/// Shows an existing folder in Explorer; `normalize_folder` refuses device, stream and
/// reserved-name forms before the path reaches the file system.
#[tauri::command]
pub async fn open_folder(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    tauri::async_runtime::spawn_blocking(move || {
        let folder = launch::normalize_folder(&path)?;
        if !std::path::Path::new(&folder).is_dir() {
            return Err(FOLDER_MISSING.to_string());
        }
        app.opener()
            .open_path(folder, None::<&str>)
            .map_err(|_| "Ordner konnte nicht geöffnet werden.".to_string())
    })
    .await
    .map_err(|_| "Ordner konnte nicht geöffnet werden.".to_string())?
}
