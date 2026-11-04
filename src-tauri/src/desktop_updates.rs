//! In-app updates. Standard unsigned builds install
//! the release's NSIS EXE after verifying its published size and SHA-256; builds with a
//! compiled-in updater key install Tauri-signed packages instead. Either way one
//! `install_update` call downloads, verifies and starts the installer, then Traygent exits.
use serde::Serialize;
use std::{
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{ipc::Channel, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use traygent_core::updates::{self, installer, Installer, UpdateInfo, CHECK_FAILED};

/// What the last successful check found; `install_update` installs only this.
#[derive(Clone)]
enum Candidate {
    Signed(Box<Update>),
    Release(Installer),
}

#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Candidate>>,
    busy: AtomicBool,
}

struct Busy<'a>(&'a AtomicBool);
impl UpdateState {
    fn begin(&self) -> Result<Busy<'_>, String> {
        self.busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "Eine Update-Aufgabe läuft bereits.")?;
        Ok(Busy(&self.busy))
    }
    fn set(&self, candidate: Option<Candidate>) -> Result<(), String> {
        *self
            .pending
            .lock()
            .map_err(|_| "Update-Dienst nicht verfügbar.")? = candidate;
        Ok(())
    }
}
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    downloaded: u64,
    total: Option<u64>,
    stage: &'static str,
}
impl DownloadProgress {
    fn stage(stage: &'static str) -> Self {
        Self {
            downloaded: 0,
            total: None,
            stage,
        }
    }
}

const SERVICE_UNAVAILABLE: &str = "Update-Dienst nicht verfügbar.";
const START_FAILED: &str = "Installation konnte nicht gestartet werden.";

fn supported() -> Result<(), String> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Ok(())
    } else {
        Err("App-Updates stehen für die Windows-x64-Version bereit.".into())
    }
}

#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    supported()?;
    let state = app.state::<UpdateState>();
    let _busy = state.begin()?;
    // A failed check never leaves an older candidate installable.
    state.set(None)?;
    let (info, candidate) = match updater_key() {
        Some(key) => check_signed(&app, key).await?,
        None => {
            let release = tauri::async_runtime::spawn_blocking(|| {
                updates::fetch_latest_release(env!("CARGO_PKG_VERSION"))
            })
            .await
            .map_err(|_| CHECK_FAILED.to_string())??;
            match release {
                Some(release) => (
                    Some(release.info),
                    Some(Candidate::Release(release.installer)),
                ),
                None => (None, None),
            }
        }
    };
    state.set(candidate)?;
    Ok(info)
}

fn updater_key() -> Option<&'static str> {
    option_env!("TRAYGENT_UPDATER_PUBLIC_KEY").filter(|key| !key.trim().is_empty())
}

/// Signed feed of builds with a compiled-in updater key.
async fn check_signed(
    app: &tauri::AppHandle,
    key: &str,
) -> Result<(Option<UpdateInfo>, Option<Candidate>), String> {
    let updater = app
        .updater_builder()
        .pubkey(key)
        .endpoints(vec![updates::ENDPOINT
            .parse()
            .map_err(|_| "Update-Adresse ungültig.")?])
        .map_err(|_| "Update-Adresse ungültig.")?
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|_| SERVICE_UNAVAILABLE)?;
    let update = updater.check().await.map_err(|error| match error {
        // A missing manifest (404) is a publishing gap, not a connection problem.
        tauri_plugin_updater::Error::ReleaseNotFound => updates::NO_SIGNED_FEED.to_string(),
        _ => CHECK_FAILED.to_string(),
    })?;
    let Some(mut update) = update else {
        return Ok((None, None));
    };
    updates::validate_update(
        update.download_url.as_str(),
        &update.version,
        env!("CARGO_PKG_VERSION"),
    )?;
    if update.signature.trim().is_empty() {
        return Err("Signatur fehlt – Update wird nicht installiert.".into());
    }
    update.timeout = Some(Duration::from_secs(300));
    let info = UpdateInfo {
        version: update.version.clone(),
        notes: update.body.clone().unwrap_or_default(),
        date: update
            .date
            .and_then(|date| updates::format_date(date.unix_timestamp())),
    };
    Ok((Some(info), Some(Candidate::Signed(Box::new(update)))))
}

/// Downloads and verifies the checked update, starts its installer and exits Traygent so
/// the installer can replace the EXE; the installer relaunches the app.
#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    on_progress: Channel<DownloadProgress>,
) -> Result<(), String> {
    supported()?;
    let state = app.state::<UpdateState>();
    let _busy = state.begin()?;
    let candidate = state
        .pending
        .lock()
        .map_err(|_| SERVICE_UNAVAILABLE)?
        .clone()
        .ok_or("Prüfe zuerst auf Updates.")?;
    match candidate {
        Candidate::Release(asset) => install_release(&app, asset, on_progress).await,
        Candidate::Signed(update) => install_signed(*update, on_progress).await,
    }
}

async fn install_release(
    app: &tauri::AppHandle,
    asset: Installer,
    on_progress: Channel<DownloadProgress>,
) -> Result<(), String> {
    let total = asset.size;
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = installer::download(&asset, |downloaded| {
            let _ = on_progress.send(DownloadProgress {
                downloaded,
                total: Some(total),
                stage: "downloading",
            });
            // The last chunk is in; size and SHA-256 are compared next.
            if downloaded == total {
                let _ = on_progress.send(DownloadProgress::stage("verifying"));
            }
        })?;
        // Only verified bytes reach the disk.
        let _ = on_progress.send(DownloadProgress::stage("installing"));
        let path = installer::stage(&std::env::temp_dir(), &asset, &bytes)?;
        // Per-user NSIS install (`installMode: currentUser`): no elevation needed.
        Command::new(path)
            .args(installer::NSIS_UPDATE_ARGS)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(drop)
            .map_err(|_| START_FAILED.to_string())
    })
    .await
    .map_err(|_| START_FAILED.to_string())??;
    // Nothing prevents `ExitRequested`, so this ends the process and frees the EXE; the
    // passive installer would otherwise close the running app itself.
    app.exit(0);
    Ok(())
}

async fn install_signed(
    update: Update,
    on_progress: Channel<DownloadProgress>,
) -> Result<(), String> {
    let mut downloaded = 0;
    let mut last_event = Instant::now();
    let verify_channel = on_progress.clone();
    let bytes = update
        .download(
            |length, total| {
                downloaded += length as u64;
                if last_event.elapsed() >= Duration::from_millis(100) || total == Some(downloaded) {
                    let _ = on_progress.send(DownloadProgress {
                        downloaded,
                        total,
                        stage: "downloading",
                    });
                    last_event = Instant::now();
                }
            },
            || {
                let _ = verify_channel.send(DownloadProgress::stage("verifying"));
            },
        )
        .await
        .map_err(|_| {
            "Download oder Signaturprüfung fehlgeschlagen – nichts installiert.".to_string()
        })?;
    // download() verifies the complete package before returning installable bytes.
    let _ = on_progress.send(DownloadProgress::stage("installing"));
    // On Windows the plugin starts the installer and exits the process itself.
    tauri::async_runtime::spawn_blocking(move || update.install(bytes))
        .await
        .map_err(|_| START_FAILED)?
        .map_err(|_| START_FAILED.to_string())
}
