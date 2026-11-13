//! Agent profiles (#33): the `sync_profiles` and `profile_login` commands and the profile
//! step of a detection. Names and arguments are the contract with `src/lib/ipc.ts`
//! (docs/SPEC.md → IPC contract).
use crate::desktop::{work, NativeState};
use tauri::AppHandle;
use traygent_core::detect::{Host, RealFs};
use traygent_core::profiles::system_linker;
use traygent_core::{AgentId, Snapshot};

/// The profile step of a detection: every profile's status is read anew, and the first
/// detection after the app started also syncs the profiles. The file system work runs
/// without the service lock; a panic in it only keeps the previous statuses.
pub(crate) fn read_or_sync(state: &NativeState) {
    let (plan, sync) = {
        let mut service = state.lock();
        (service.profile_plan(), service.take_startup_profile_sync())
    };
    let outcome = std::panic::catch_unwind(|| {
        if sync {
            let linker = system_linker(&Host::current());
            plan.run(linker.as_ref(), chrono::Utc::now())
        } else {
            plan.read()
        }
    });
    if let Ok(outcome) = outcome {
        state.lock().apply_profiles(outcome);
    }
}

/// Syncs every profile of every agent now and returns the snapshot with their statuses.
#[tauri::command]
pub async fn sync_profiles(app: AppHandle) -> Result<Snapshot, String> {
    work(app, |service| {
        let linker = system_linker(&Host::current());
        service.sync_profiles(linker.as_ref(), chrono::Utc::now());
        Ok(service.snapshot())
    })
    .await
}

/// Opens a terminal that signs the agent's profile in; only ever on an explicit click.
#[tauri::command]
pub async fn profile_login(app: AppHandle, agent: AgentId, profile: String) -> Result<(), String> {
    work(app, move |service| {
        let (plan, fallback) = service.prepare_login(agent, &profile, &Host::current(), &RealFs)?;
        crate::desktop_agents::start(&plan, fallback.as_ref())
    })
    .await
}
