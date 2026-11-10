//! Background updates of the local agent CLIs (#16): the schedule thread, one update run
//! and the `update_agents` command. Names and arguments are the contract with
//! `src/lib/ipc.ts` (docs/SPEC.md → IPC contract).
//!
//! The process list and the update commands run on the run's own thread without the
//! service lock; the service only commits each status, and every change is emitted.
use crate::desktop::{work, NativeState, FAILED};
use crate::desktop_agents::SNAPSHOT_EVENT;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use traygent_core::agent_updates::{self, Runners};
use traygent_core::detect::{Candidate, Host, RealFs, SystemRunner};
use traygent_core::{AgentId, AgentUpdateState, AgentUpdateStatus, Snapshot};

fn emit(app: &AppHandle, snapshot: &Snapshot) {
    if let Err(error) = app.emit(SNAPSHOT_EVENT, snapshot) {
        eprintln!("Traygent: {error}");
    }
}

/// Commits one agent's status under a short lock and emits the snapshot.
fn report(app: &AppHandle, status: AgentUpdateStatus) {
    let state = app.state::<NativeState>();
    let mut service = state.lock();
    service.set_agent_update(status);
    let snapshot = service.snapshot();
    drop(service);
    emit(app, &snapshot);
}

/// Starts the schedule thread: once a minute it starts a run when the setting is on and one
/// is due, so switching the setting on leads to a run within a minute.
pub(crate) fn start_schedule(app: &AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("traygent-agent-updates".into())
        .spawn(move || schedule(&app));
    if let Err(error) = spawned {
        eprintln!("Traygent: {error}");
    }
}

fn schedule(app: &AppHandle) {
    let started = Instant::now();
    loop {
        std::thread::sleep(agent_updates::TICK);
        let state = app.state::<NativeState>();
        let mut service = state.lock();
        if !service.agent_updates_due(started, Instant::now()) {
            continue;
        }
        // `None` while a detection runs or before the first one (the next tick retries) and
        // without a local CLI (counted as a run, so the next try is an interval later).
        let Some(targets) = service.start_scheduled_agent_updates(Instant::now()) else {
            continue;
        };
        let snapshot = service.snapshot();
        drop(service);
        emit(app, &snapshot);
        run(app, &targets);
    }
}

/// Runs the update run `service` already marked as begun and ends it on every path: a
/// panic leaves neither the run flag nor a `running` status behind. A detection follows every
/// run, so versions and paths refresh; one that ran meanwhile may have probed a CLI while it
/// was being replaced, so it reruns.
fn run(app: &AppHandle, targets: &[(AgentId, Candidate)]) {
    let finished = catch_unwind(AssertUnwindSafe(|| update_all(app, targets)));
    if finished.is_err() {
        eprintln!("Traygent: agent update run panicked");
    }
    let state = app.state::<NativeState>();
    let mut service = state.lock();
    service.finish_agent_updates(Instant::now());
    if service.begin_or_rerun_detection() {
        crate::desktop_agents::run_detection(app, &mut service);
    }
    let snapshot = service.snapshot();
    drop(service);
    emit(app, &snapshot);
}

/// One agent after the other: `running`, then the process list and the update command.
/// The process list is read again right before each agent, since the previous agent's
/// update may have taken minutes.
fn update_all(app: &AppHandle, targets: &[(AgentId, Candidate)]) {
    let host = Host::current();
    let list = SystemRunner {
        timeout: agent_updates::LIST_TIMEOUT,
        ..SystemRunner::default()
    };
    let probe = SystemRunner::default();
    // A timed-out npm shim must not leave its node updater running.
    let update = SystemRunner {
        timeout: agent_updates::UPDATE_TIMEOUT,
        kill_tree: true,
    };
    let runners = Runners {
        list: &list,
        probe: &probe,
        update: &update,
    };
    for (agent, cli) in targets {
        let running = AgentUpdateState::Running;
        report(
            app,
            agent_updates::status(*agent, running, chrono::Utc::now()),
        );
        let status = agent_updates::run_agent(*agent, cli, &host, &RealFs, &runners, &|| {
            chrono::Utc::now()
        });
        report(app, status);
    }
}

/// Starts a run over every ready local CLI now, whatever the setting says, and returns at
/// once. Each status follows as a snapshot event.
#[tauri::command]
pub async fn update_agents(app: AppHandle) -> Result<Snapshot, String> {
    let handle = app.clone();
    work(app, move |service| {
        let targets = service.start_agent_updates()?;
        let spawned = std::thread::Builder::new()
            .name("traygent-agent-update-run".into())
            .spawn(move || run(&handle, &targets));
        if spawned.is_err() {
            service.abort_agent_updates();
            return Err(FAILED.to_string());
        }
        Ok(service.snapshot())
    })
    .await
}
