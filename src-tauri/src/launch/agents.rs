//! Per-agent CLI arguments for permission, model, effort and session
//! (docs/AGENT_RECIPES.md §1–§3, order as listed there; extra args always last).
use crate::models::{AgentId, EnvId, LaunchOptions, PermissionId, SessionMode};

fn set(value: &str) -> Option<&str> {
    (value != "default").then_some(value)
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

/// Order: permission · model · effort · session · extra.
pub fn claude(options: &LaunchOptions, session: SessionMode, extra: Vec<String>) -> Vec<String> {
    // Outside bypass the starter keeps `--allow-dangerously-skip-permissions`, so Shift+Tab can
    // still reach bypass. `default` instead of the `manual` alias, which needs CLI >= 2.1.200.
    let mut args = match options.permission {
        PermissionId::Ask => strings(&[
            "--permission-mode",
            "default",
            "--allow-dangerously-skip-permissions",
        ]),
        PermissionId::Auto => strings(&[
            "--permission-mode",
            "auto",
            "--allow-dangerously-skip-permissions",
        ]),
        PermissionId::Bypass => strings(&["--dangerously-skip-permissions"]),
    };
    if let Some(model) = set(&options.model) {
        args.extend(strings(&["--model", model]));
    }
    if let Some(effort) = set(&options.effort) {
        args.extend(strings(&["--effort", effort]));
    }
    match session {
        SessionMode::New => {}
        SessionMode::Continue => args.push("--continue".into()),
        SessionMode::Resume => args.push("--resume".into()),
    }
    args.extend(extra);
    args
}

/// Order: `--standalone` (local only; the Docker init adds it) · `--auto` · `--continue` · extra.
/// Model and the bypass rule travel in `OPENCODE_CONFIG_CONTENT` ([`opencode_config`]).
pub fn opencode(
    options: &LaunchOptions,
    session: SessionMode,
    extra: Vec<String>,
    env: EnvId,
) -> Vec<String> {
    // Without --standalone the CLI attaches to a background service that keeps the env of its
    // own first start and ignores this launch's config (agent-tools ADR-0005).
    let mut args = Vec::new();
    if env == EnvId::Local {
        args.push("--standalone".to_string());
    }
    if options.permission != PermissionId::Ask {
        args.push("--auto".into());
    }
    if session == SessionMode::Continue {
        args.push("--continue".into());
    }
    args.extend(extra);
    args
}

/// JSON for `OPENCODE_CONFIG_CONTENT`: `model` as `provider/model#variant` (the variant only
/// with a model), plus the allow-all rule for bypass. Override precedence over the user's own
/// `model` key and how rule arrays of layers merge are UNVERIFIED (docs/AGENT_RECIPES.md §2).
pub fn opencode_config(options: &LaunchOptions) -> Option<String> {
    let mut config = serde_json::Map::new();
    if let Some(model) = set(&options.model) {
        let model = match set(&options.effort) {
            Some(effort) => format!("{model}#{effort}"),
            None => model.to_string(),
        };
        config.insert("model".into(), model.into());
    }
    if options.permission == PermissionId::Bypass {
        let rule = serde_json::json!([{ "action": "*", "resource": "*", "effect": "allow" }]);
        config.insert("permissions".into(), rule);
    }
    (!config.is_empty()).then(|| serde_json::Value::Object(config).to_string())
}

/// Order: [`resume` [`--last`]] · permission · `-m` · `-c model_reasoning_effort` · extra.
pub fn codex(
    options: &LaunchOptions,
    session: SessionMode,
    extra: Vec<String>,
    env: EnvId,
) -> Vec<String> {
    let mut args = match session {
        SessionMode::New => Vec::new(),
        SessionMode::Continue => strings(&["resume", "--last"]),
        SessionMode::Resume => strings(&["resume"]),
    };
    // Docker's default seccomp profile breaks Codex's Linux sandbox; its docs say to run
    // `danger-full-access` inside a container, so there `ask` only asks on request.
    let docker = env == EnvId::Docker;
    args.extend(match (options.permission, docker) {
        (PermissionId::Ask, false) => {
            strings(&["--sandbox", "read-only", "--ask-for-approval", "on-request"])
        }
        (PermissionId::Ask, true) => strings(&[
            "--sandbox",
            "danger-full-access",
            "--ask-for-approval",
            "on-request",
        ]),
        (PermissionId::Auto, false) => strings(&["--approve-for-me"]),
        // UNVERIFIED combination (docs/AGENT_RECIPES.md §3).
        (PermissionId::Auto, true) => strings(&[
            "--sandbox",
            "danger-full-access",
            "--ask-for-approval",
            "on-request",
            "-c",
            "approvals_reviewer=auto_review",
        ]),
        (PermissionId::Bypass, _) => strings(&["--dangerously-bypass-approvals-and-sandbox"]),
    });
    if let Some(model) = set(&options.model) {
        args.extend(strings(&["-m", model]));
    }
    if let Some(effort) = set(&options.effort) {
        args.extend(["-c".to_string(), format!("model_reasoning_effort={effort}")]);
    }
    args.extend(extra);
    args
}

/// The agent's own arguments for this start.
pub fn args(
    agent: AgentId,
    env: EnvId,
    options: &LaunchOptions,
    session: SessionMode,
    extra: Vec<String>,
) -> Vec<String> {
    match agent {
        AgentId::Claude => claude(options, session, extra),
        AgentId::Opencode => opencode(options, session, extra, env),
        AgentId::Codex => codex(options, session, extra, env),
    }
}

#[cfg(test)]
mod tests;
