//! The sign-in of a profile (#33, #41): hidden by default, in a visible terminal with
//! „Debug“ on. Both run the same call: the profile's environment, then the CLI by absolute
//! path with the recipe's login arguments (docs/SPEC.md → Profiles).
use super::quote::{encode_command, invoke, ps_literal};
use super::{
    is_windows_absolute, profile_env, tab, wrap, LaunchPlan, Resolved, COMMAND_TOO_LONG,
    MAX_ENCODED_CHARS,
};
use crate::detect::Candidate;
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

/// A hidden call of the CLI: the sign-in without a window (#41, „Debug“ off) and the quota
/// read of Claude Code (#110). The call runs in a hidden
/// `powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand`, whose exit code is
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
    hidden(resolved, home, || login_call(agent, resolved))
}

/// Claude Code's own quota read (#110): `/usage` in print mode, as JSON, without a session
/// file. No `--bare`: it may change which sign-in the CLI uses.
pub const USAGE_ARGS: [&str; 5] = [
    "-p",
    "/usage",
    "--output-format",
    "json",
    "--no-session-persistence",
];

/// The variables that replace Claude Code's stored sign-in. The quota read of a local
/// session refuses them beforehand (`quota_paths.rs`); the read itself always removes them,
/// so a Docker session's read on the host never answers for them.
pub const CLAUDE_LOGIN_OVERRIDES: [&str; 7] = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_CUSTOM_OAUTH_URL",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
];

/// The hidden `/usage` read of Claude Code in `cwd` (#110): the CLI by absolute path with
/// [`USAGE_ARGS`], in the profile's environment as a local start sets it (`profiled`: the
/// agent has extra profiles, so the default one removes an inherited `CLAUDE_CONFIG_DIR`).
/// stdout is UTF-8, so the JSON reaches the caller unchanged also when PowerShell relays it.
pub fn hidden_usage_plan(
    resolved: &Resolved,
    cwd: &str,
    profiled: bool,
) -> Result<HiddenLogin, String> {
    hidden(resolved, cwd, || {
        let agent = AgentId::Claude;
        let cli = cli_path(agent, resolved)?;
        // No character an npm `.cmd` shim would interpret (`settings::check_shim_args`).
        let args = USAGE_ARGS.map(String::from);
        let mut script = vec!["[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)".into()];
        script.extend(
            CLAUDE_LOGIN_OVERRIDES
                .map(|name| format!("Remove-Item Env:{name} -ErrorAction SilentlyContinue")),
        );
        script.extend(profile_env(agent, resolved, profiled)?);
        script.push(invoke(&cli.path, &args));
        Ok(script)
    })
}

fn hidden(
    resolved: &Resolved,
    cwd: &str,
    call: impl FnOnce() -> Result<Vec<String>, String>,
) -> Result<HiddenLogin, String> {
    if !is_windows_absolute(&resolved.powershell) {
        return Err("Der Pfad zu PowerShell ist kein absoluter Windows-Pfad.".into());
    }
    let mut script = vec!["$LASTEXITCODE=$null".to_string()];
    script.extend(call()?);
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
        cwd: cwd.into(),
    })
}

/// The agent's CLI by absolute Windows path, for both hidden calls and the visible sign-in.
fn cli_path(agent: AgentId, resolved: &Resolved) -> Result<&Candidate, String> {
    let label = agent.label();
    let cli = resolved
        .cli
        .as_ref()
        .ok_or_else(|| format!("{label} ist nicht installiert."))?;
    if !is_windows_absolute(&cli.path) {
        return Err(format!(
            "Der Pfad zu {label} ist kein absoluter Windows-Pfad."
        ));
    }
    Ok(cli)
}

/// The profile's environment and the CLI's login call, shared by both sign-ins.
fn login_call(agent: AgentId, resolved: &Resolved) -> Result<Vec<String>, String> {
    let label = agent.label();
    let recipe = crate::profiles::recipe(agent)
        .ok_or_else(|| format!("{label} unterstützt keine Profile."))?;
    let cli = cli_path(agent, resolved)?;
    let args: Vec<String> = recipe
        .login_args
        .iter()
        .map(|arg| arg.to_string())
        .collect();
    let mut script: Vec<String> = profile_env(agent, resolved, true)?.into_iter().collect();
    script.extend(HEADER_ROOM_VARS.map(header_room));
    script.push(invoke(&cli.path, &args));
    Ok(script)
}

/// The CLI's sign-in ends on `http://localhost:<port>/callback`, and the browser sends every
/// cookie any local site ever set for `localhost` along (cookies ignore the port). Past the
/// runtime's 16 KiB header limit the CLI answers 431 and never sees the code (#89), so both
/// runtimes the CLI ships on get room for them — appended, so the user's own options stay.
const HEADER_ROOM: &str = "--max-http-header-size=1048576";
const HEADER_ROOM_VARS: [&str; 2] = ["BUN_OPTIONS", "NODE_OPTIONS"];

fn header_room(name: &str) -> String {
    let flag = ps_literal(&format!(" {HEADER_ROOM}"));
    format!("$env:{name}=($env:{name}+{flag}).Trim()")
}
