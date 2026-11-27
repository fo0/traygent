//! Quota for one selected account. Only the sanitized provider result crosses IPC.
use crate::desktop::{NativeState, FAILED};
use tauri::{AppHandle, Manager};
use traygent_core::detect::{Host, RealFs};
use traygent_core::quota::{self, AgentQuota, CliRead, Source};
use traygent_core::quota_paths::{self, QuotaSource};
use traygent_core::{AgentId, EnvId};

#[tauri::command]
pub async fn get_agent_quota(
    app: AppHandle,
    agent: AgentId,
    profile: Option<String>,
    env: EnvId,
) -> Result<AgentQuota, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<NativeState>();
        let settings = state.lock().settings().clone();
        // Off (`quotaReads`, ADR-0014): returns before anything is resolved or started.
        quota::read(agent, profile.as_deref(), &settings, || {
            let host = Host::current();
            match quota_paths::quota_source(agent, profile.as_deref(), env, &settings, &host)? {
                QuotaSource::File(path) => Ok(Source::File(path)),
                // The host's CLI for every environment (ADR-0015). The read is counted under
                // the same lock that checked no update replaces the CLI (`agent_updates`).
                QuotaSource::ClaudeCli { profile_dir } => {
                    let service = state.lock();
                    let plan = service.prepare_quota_read(agent, profile_dir, &host, &RealFs)?;
                    let read = CliRead::new(plan);
                    drop(service);
                    Ok(Source::Cli(read))
                }
            }
        })
    })
    .await
    .map_err(|_| FAILED.to_string())
}
