//! The GitHub IPC commands (docs/adr/0005): connect with a token, import a gh sign-in,
//! disconnect, and the repository listing. Names and arguments are the contract with
//! `src/lib/ipc.ts` (docs/SPEC.md → IPC contract).
//!
//! `/user`, gh, the vault reads and writes and `/user/repos` run on worker threads without
//! the service lock; the service only commits their results. Only the local vault delete of
//! a replaced or disconnected account runs under it (no network, no process).
use crate::desktop::{work, NativeState, FAILED};
use crate::desktop_agents::SNAPSHOT_EVENT;
use tauri::{AppHandle, Emitter, Manager};
use traygent_core::detect::Host;
use traygent_core::github::{self, GhCli, HttpTransport, KeyringVault, SecretToken};
use traygent_core::service::{Listing, Service};
use traygent_core::{GithubAccount, LocalAccounts, Snapshot};

const LISTING_FAILED: &str = "Repositories konnten nicht geladen werden.";

/// Runs `operation` on a blocking worker thread without the service lock.
async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|_| FAILED.to_string())?
}

/// The connected account, read under a short lock.
fn connected(app: &AppHandle) -> Option<GithubAccount> {
    app.state::<NativeState>().lock().github_account().cloned()
}

/// Commits a verified, vaulted account and starts its listing.
async fn commit(app: AppHandle, account: GithubAccount) -> Result<Snapshot, String> {
    let handle = app.clone();
    work(app, move |service| {
        let listing = service.connect_github(account, &KeyringVault::new());
        run_listing(&handle, service, listing);
        Ok(service.snapshot())
    })
    .await
}

/// Starts a listing of the connected account on its own thread unless one runs; the
/// finished snapshot is emitted.
pub(crate) fn start_listing(app: &AppHandle, service: &mut Service) {
    if let Some(listing) = service.begin_listing() {
        run_listing(app, service, listing);
    }
}

/// Runs the listing `service` already marked as begun; `loading` is cleared on every path.
fn run_listing(app: &AppHandle, service: &mut Service, listing: Listing) {
    let app = app.clone();
    let running = listing.clone();
    let spawned = std::thread::Builder::new()
        .name("traygent-github".into())
        .spawn(move || listing_thread(&app, running));
    if spawned.is_err() {
        service.abort_listing(&listing);
    }
}

fn listing_thread(app: &AppHandle, listing: Listing) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let api = HttpTransport::new()?;
        github::list_repos(&listing.account, &KeyringVault::new(), &api)
    }))
    .unwrap_or_else(|_| Err(LISTING_FAILED.into()));
    let state = app.state::<NativeState>();
    let finished = state
        .lock()
        .finish_listing(listing, result, chrono::Utc::now());
    // `None`: the account changed meanwhile; its own listing reports.
    if let Some(snapshot) = finished {
        if let Err(error) = app.emit(SNAPSHOT_EVENT, &snapshot) {
            eprintln!("Traygent: {error}");
        }
    }
}

/// Verifies a pasted personal access token, vaults it and lists the repositories.
#[tauri::command]
pub async fn github_connect_token(app: AppHandle, token: String) -> Result<Snapshot, String> {
    let account = blocking(move || {
        let api = match HttpTransport::new() {
            Ok(api) => api,
            Err(error) => {
                // Wiped on drop, like every token.
                drop(SecretToken::new(token));
                return Err(error);
            }
        };
        github::connect_token(token, &KeyringVault::new(), &api)
    })
    .await?;
    commit(app, account).await
}

/// The local gh sign-ins for github.com; reads no token.
#[tauri::command]
pub async fn github_local_accounts(app: AppHandle) -> Result<LocalAccounts, String> {
    blocking(move || {
        let login = connected(&app).map(|account| account.login);
        let gh = GhCli::installed(&Host::current());
        Ok(github::local_accounts(&gh, login.as_deref()))
    })
    .await
}

/// Imports the token of the gh sign-in `login` after the user's explicit „Übernehmen“.
#[tauri::command]
pub async fn github_import_local(app: AppHandle, login: String) -> Result<Snapshot, String> {
    let account = blocking(move || {
        let api = HttpTransport::new()?;
        let gh = GhCli::installed(&Host::current());
        github::import_local(&login, &gh, &KeyringVault::new(), &api)
    })
    .await?;
    commit(app, account).await
}

/// Deletes Traygent's vault copy of the token; gh stays signed in.
#[tauri::command]
pub async fn github_disconnect(app: AppHandle) -> Result<Snapshot, String> {
    work(app, |service| {
        service.disconnect_github(&KeyringVault::new())
    })
    .await
}

/// Returns at once with `github.loading: true`; the finished snapshot follows as an event.
#[tauri::command]
pub async fn github_refresh_repos(app: AppHandle) -> Result<Snapshot, String> {
    let handle = app.clone();
    work(app, move |service| {
        start_listing(&handle, service);
        Ok(service.snapshot())
    })
    .await
}
