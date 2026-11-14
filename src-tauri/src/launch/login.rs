//! The sign-in of a profile (#33, #41): hidden by default, in a visible terminal with
//! „Debug“ on. Both run the same call: the profile's environment, then the CLI by absolute
//! path with the recipe's login arguments (docs/SPEC.md → Profiles).
use super::quote::{encode_command, invoke, ps_literal};
use super::{
    is_windows_absolute, profile_env, tab, wrap, LaunchPlan, Resolved, COMMAND_TOO_LONG,
    MAX_ENCODED_CHARS,
};
use crate::models::{AgentId, AppSettings};

/// Opens a terminal in `home` that signs the agent's profile in: the CLI by absolute path
/// with the recipe's login arguments, in the profile's environment. Only with „Debug“ on
/// (#41): it stays open like the install terminal, so every line of the CLI can be read.
pub fn login_plan(
    agent: AgentId,
    profile_label: &str,
    settings: &AppSettings,
    resolved: &Resolved,
    home: &str,
) -> Result<LaunchPlan, String> {
    let title = format!("{} · {profile_label} anmelden", agent.label());
    let mut script = vec![format!("$Host.UI.RawUI.WindowTitle={}", ps_literal(&title))];
    script.extend(login_call(agent, resolved)?);
    wrap(
        &title,
        home,
        &script.join("; "),
        &resolved.powershell,
        tab(settings, resolved),
        false,
    )
}

/// A sign-in without a window (#41, „Debug“ off): the same call as [`login_plan`] in a
/// hidden `powershell.exe -NoProfile -NonInteractive -EncodedCommand`, whose exit code is
/// the CLI's — 1 when the CLI could not run at all (`$LASTEXITCODE` unset).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HiddenLogin {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
}

pub fn hidden_login_plan(
    agent: AgentId,
    resolved: &Resolved,
    home: &str,
) -> Result<HiddenLogin, String> {
    if !is_windows_absolute(&resolved.powershell) {
        return Err("Der Pfad zu PowerShell ist kein absoluter Windows-Pfad.".into());
    }
    let mut script = vec!["$LASTEXITCODE=$null".to_string()];
    script.extend(login_call(agent, resolved)?);
    script.push("if ($null -eq $LASTEXITCODE) { exit 1 }".into());
    script.push("exit $LASTEXITCODE".into());
    let encoded = encode_command(&script.join("; "));
    if encoded.len() > MAX_ENCODED_CHARS {
        return Err(COMMAND_TOO_LONG.into());
    }
    let flags = [
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-EncodedCommand",
    ];
    Ok(HiddenLogin {
        program: resolved.powershell.clone(),
        args: flags
            .into_iter()
            .map(String::from)
            .chain([encoded])
            .collect(),
        cwd: home.into(),
    })
}

/// The profile's environment and the CLI's login call, shared by both sign-ins.
fn login_call(agent: AgentId, resolved: &Resolved) -> Result<Vec<String>, String> {
    let label = agent.label();
    let recipe = crate::profiles::recipe(agent)
        .ok_or_else(|| format!("{label} unterstützt keine Profile."))?;
    let cli = resolved
        .cli
        .as_ref()
        .ok_or_else(|| format!("{label} ist nicht installiert."))?;
    if !is_windows_absolute(&cli.path) {
        return Err(format!(
            "Der Pfad zu {label} ist kein absoluter Windows-Pfad."
        ));
    }
    let args: Vec<String> = recipe
        .login_args
        .iter()
        .map(|arg| arg.to_string())
        .collect();
    let mut script: Vec<String> = profile_env(agent, resolved, true)?.into_iter().collect();
    script.push(invoke(&cli.path, &args));
    Ok(script)
}
