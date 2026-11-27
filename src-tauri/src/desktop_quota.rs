//! Quota for one selected account. Only the sanitized provider result crosses IPC.
use crate::desktop::{NativeState, FAILED};
use tauri::{AppHandle, Manager};
use traygent_core::detect::Host;
use traygent_core::quota::{self, AgentQuota};
use traygent_core::{quota_paths, AgentId, EnvId};

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
        // Off (`quotaReads`, ADR-0014): returns before the path is resolved.
        quota::read(agent, profile.as_deref(), &settings, || {
            quota_paths::credentials_path(
                agent,
                profile.as_deref(),
                env,
                &settings,
                &Host::current(),
            )
        })
    })
    .await
    .map_err(|_| FAILED.to_string())
}
