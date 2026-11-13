//! Agent profiles (#33): the `sync_profiles` and `profile_login` commands, the profile
//! step of a detection and the background sync thread. Names and arguments are the contract
//! with `src/lib/ipc.ts` (docs/SPEC.md → IPC contract).
use crate::desktop::{work, NativeState, FAILED};
use crate::desktop_agents::SNAPSHOT_EVENT;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use traygent_core::detect::{Host, RealFs};
use traygent_core::profiles::{system_linker, SyncOutcome, SyncPlan};
use traygent_core::service::PROFILE_SYNC_TICK;
use traygent_core::{AgentId, Snapshot};

/// The profile step of a detection: every profile's status is read anew, and the first
/// detection after the app started also syncs the profiles. The file system work runs
/// without the service lock; a panic in it only keeps the previous statuses.
pub(crate) fn read_or_sync(state: &NativeState) {
    let (plan, sync) = {
        let mut service = state.lock();
        (service.profile_plan(), service.take_startup_profile_sync())
    };
    let outcome = std::panic::catch_unwind(|| if sync { run(&plan) } else { plan.read() });
    if let Ok(outcome) = outcome {
        state.lock().apply_profiles(outcome);
    }
}

/// Starts the background sync thread (#37): once a minute it asks the service whether a
/// sync is due ([`PROFILE_SYNC_INTERVAL`](traygent_core::service::PROFILE_SYNC_INTERVAL)
/// after the last one) and runs it like the others: the plan under the lock, the file
/// system work without it, then the commit and a snapshot event for an open panel.
pub(crate) fn start_schedule(app: &AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("traygent-profile-sync".into())
        .spawn(move || schedule(&app));
    if let Err(error) = spawned {
        eprintln!("Traygent: {error}");
    }
}

fn schedule(app: &AppHandle) {
    let started = Instant::now();
    loop {
        std::thread::sleep(PROFILE_SYNC_TICK);
        let state = app.state::<NativeState>();
        let plan = {
            let service = state.lock();
            // Another sync in flight would leave this plan stale behind it, then repeat its
            // work: skip the tick; that sync's commit restarts the interval.
            if !service.profile_sync_due(started, Instant::now())
                || traygent_core::profiles::sync::busy()
            {
                continue;
            }
            service.profile_plan()
        };
        let outcome = std::panic::catch_unwind(|| run(&plan));
        let mut service = state.lock();
        // Silent: what could not be done shows only in each profile's `syncError`. A panic
        // keeps the previous statuses and still waits the full interval.
        match outcome {
            Ok(outcome) => {
                service.apply_profiles(outcome);
            }
            Err(_) => service.skip_profile_sync(Instant::now()),
        }
        let snapshot = service.snapshot();
        drop(service);
        if let Err(error) = app.emit(SNAPSHOT_EVENT, &snapshot) {
            eprintln!("Traygent: {error}");
        }
    }
}

/// Syncs a plan's profiles with the system linker, stamped with the current time.
fn run(plan: &SyncPlan) -> SyncOutcome {
    let linker = system_linker(&Host::current());
    plan.run(linker.as_ref(), chrono::Utc::now())
}

/// Runs a profile plan on a blocking worker thread without the service lock: a sync (the
/// hidden linker call, a first adoption of large directories) or only the status read.
/// Sync runs still take turns among themselves.
pub(crate) async fn run_plan(plan: SyncPlan, sync: bool) -> Result<SyncOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || if sync { run(&plan) } else { plan.read() })
        .await
        .map_err(|_| FAILED.to_string())
}

/// Syncs every profile of every agent now and returns the snapshot with their statuses.
#[tauri::command]
pub async fn sync_profiles(app: AppHandle) -> Result<Snapshot, String> {
    let plan = work(app.clone(), |service| Ok(service.profile_plan())).await?;
    let outcome = run_plan(plan, true).await?;
    work(app, move |service| {
        service.apply_profiles(outcome);
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
