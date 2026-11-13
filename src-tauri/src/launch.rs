//! The launch builder: a pure function from a request to the argv of a terminal process
//! (docs/SPEC.md → Launch rules, docs/AGENT_RECIPES.md §0/§5).
//!
//! The terminal always runs `powershell.exe -NoLogo -NoExit -EncodedCommand <Base64>`, the
//! script as UTF-16LE in Base64, so no `;`, `%`, quote or space of it reaches wt.exe or
//! PowerShell's argument parsing. The script sets the title and location and calls the CLI by
//! absolute path with every argument a single-quoted literal. No native argv element ever
//! contains `"` or is empty. `-NoExit` keeps an error before or at the start visible; a normal
//! end of the agent closes the terminal ([`run_agent`]).
use crate::catalog;
use crate::detect::Candidate;
use crate::models::{AgentId, AppSettings, EnvId, LaunchRequest, TerminalId};
use crate::{ports, settings};

pub mod agents;
pub mod docker;
pub mod folder;
pub mod quote;

pub(crate) use folder::is_windows_absolute;
pub use folder::{is_network_path, normalize_folder};

use quote::{encode_command, invoke, ps_literal, set_env_base64};

/// Windows caps a command line at 32767 characters; the program and wt options need room too.
const MAX_ENCODED_CHARS: usize = 30_000;
const COMMAND_TOO_LONG: &str =
    "Der Startbefehl ist zu lang. Bitte einen kürzeren Ordnerpfad oder weniger Zusatzargumente wählen.";
pub const WSL_SOON: &str = "WSL ist noch nicht verfügbar.";
const DOCKER_MISSING: &str = "Docker ist nicht installiert.";
pub const GIT_MISSING: &str = "Git ist nicht installiert.";
const NO_REPO: &str = "Kein Repository angegeben.";
const PROFILE_UNRESOLVED: &str = "Der Profilordner wurde nicht ermittelt.";
pub const CLONE_FAILED: &str =
    "[traygent] Das Repository konnte nicht geklont werden. Bitte die Meldungen oben prüfen.";
const DOCKER_NETWORK_PATH: &str =
    "Docker kann keine Netzwerk- oder WSL-Pfade einbinden. Bitte einen lokalen Ordner wählen.";
/// The Windows Terminal window every tab opens in when `sharedWindow` is on.
pub const SHARED_WINDOW: &str = "traygent";
/// Exit codes of a normal end: success, SIGINT (128 + 2) and STATUS_CONTROL_C_EXIT
/// (0xC000013A as the signed `$LASTEXITCODE`).
const NORMAL_EXIT: &str = "0,130,-1073741510";

/// One process to start: the terminal program, its argv, the working dir and the title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub title: String,
    /// The window it opens: `console` needs `CREATE_NEW_CONSOLE` when spawned.
    pub terminal: TerminalId,
}

/// What the host provides for a launch, resolved by the caller (file system, detection).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resolved {
    /// The local CLI of the requested agent.
    pub cli: Option<Candidate>,
    pub docker: Option<String>,
    /// gh.exe, only consulted when `docker.ghToken` is on.
    pub gh: Option<String>,
    /// The host `.gitconfig`, only when it exists as a file.
    pub gitconfig: Option<String>,
    /// git.exe, only resolved when a repository start has to clone first.
    pub git: Option<String>,
    pub powershell: String,
    /// wt.exe when installed; without it every terminal is a console.
    pub wt: Option<String>,
    /// Short random suffix for the container name.
    pub nonce: String,
    /// The config directory of the requested profile (local starts); `None` = the default
    /// profile, the CLI's own home.
    pub profile_dir: Option<String>,
}

/// The agent's mark in a tab title: Claude Code's own title mark, OpenCode's square logo,
/// Codex's prompt mark.
pub fn glyph(agent: AgentId) -> char {
    match agent {
        AgentId::Claude => '\u{2733}',
        AgentId::Opencode => '\u{25A3}',
        AgentId::Codex => '\u{276F}',
    }
}

/// `<glyph> <folder name>`; for a repository start the folder name is the repository's.
pub fn title(agent: AgentId, folder: &str) -> String {
    let leaf = folder.rsplit('\\').find(|part| !part.is_empty());
    format!("{} {}", glyph(agent), leaf.unwrap_or(folder))
}

/// The terminal for a start in an existing folder.
pub fn plan(
    request: &LaunchRequest,
    settings: &AppSettings,
    resolved: &Resolved,
) -> Result<LaunchPlan, String> {
    build(request, settings, resolved, None)
}

/// The terminal for a start in `request.repo` whose folder (`request.folder`, derived by the
/// caller) does not exist yet: it opens in `root`, the clone root the caller creates, and the
/// script clones into the folder before it enters it. A failed clone ends the script.
pub fn clone_plan(
    request: &LaunchRequest,
    settings: &AppSettings,
    resolved: &Resolved,
    root: &str,
) -> Result<LaunchPlan, String> {
    build(request, settings, resolved, Some(root))
}

fn build(
    request: &LaunchRequest,
    settings: &AppSettings,
    resolved: &Resolved,
    clone_root: Option<&str>,
) -> Result<LaunchPlan, String> {
    let agent = request.agent;
    let label = agent.label();
    let entry = catalog::entry(agent);
    if request.env == EnvId::Wsl {
        return Err(WSL_SOON.into());
    }
    if !entry.environments.contains(&request.env) {
        return Err(format!("{label} kann in dieser Umgebung nicht starten."));
    }
    if !entry.sessions.contains(&request.session) {
        return Err(format!("{label} unterstützt diesen Session-Modus nicht."));
    }
    let folder = normalize_folder(&request.folder)?;
    let options = settings::normalize_options(request.options.clone());
    settings::validate_options(agent, &options)?;
    let extra = settings::extra_args(agent, &options.extra_args)?;
    let ports = request_ports(request)?;
    let title = title(agent, &folder);
    let mut script = vec![format!("$Host.UI.RawUI.WindowTitle={}", ps_literal(&title))];
    let cwd = match clone_root {
        Some(root) => {
            script.extend(clone(request, &folder, resolved)?);
            normalize_folder(root)?
        }
        None => folder.clone(),
    };
    // `-ErrorAction Stop` ends the script when the folder is gone or unreachable: with
    // -NoExit the user lands at a prompt instead of an agent running in the wrong folder.
    script.push(format!(
        "Set-Location -LiteralPath {} -ErrorAction Stop",
        ps_literal(&folder)
    ));
    let config = (agent == AgentId::Opencode)
        .then(|| agents::opencode_config(&options))
        .flatten();
    let argv = agents::args(agent, request.env, &options, request.session, extra);
    match request.env {
        EnvId::Local => {
            let cli = resolved
                .cli
                .as_ref()
                .ok_or_else(|| format!("{label} ist nicht installiert."))?;
            if !is_windows_absolute(&cli.path) {
                return Err(format!(
                    "Der Pfad zu {label} ist kein absoluter Windows-Pfad."
                ));
            }
            if cli.shim {
                settings::check_shim_args(agent, &argv)?;
            }
            if let Some(config) = &config {
                script.push(set_env_base64("OPENCODE_CONFIG_CONTENT", config));
            }
            let profiled = !settings.profiles.get(agent).is_empty();
            let wanted = request.profile.as_deref();
            let extra = wanted.is_some_and(|id| id != crate::models::DEFAULT_PROFILE);
            if extra != resolved.profile_dir.is_some() {
                return Err(PROFILE_UNRESOLVED.into());
            }
            script.extend(profile_env(agent, resolved, profiled)?);
            if agent == AgentId::Claude {
                // Claude Code would replace the title with its own.
                script.push("$env:CLAUDE_CODE_DISABLE_TERMINAL_TITLE='1'".into());
            }
            script.push(run_agent(label, &[invoke(&cli.path, &argv)], &[]));
        }
        EnvId::Docker | EnvId::Wsl => {
            if is_network_path(&folder) {
                return Err(DOCKER_NETWORK_PATH.into());
            }
            let docker = resolved.docker.as_deref().ok_or(DOCKER_MISSING)?;
            if !is_windows_absolute(docker) {
                return Err("Der Pfad zu Docker ist kein absoluter Windows-Pfad.".into());
            }
            let run = docker::run(
                agent,
                &folder,
                argv,
                config.as_deref(),
                &ports,
                settings,
                resolved,
            )?;
            script.extend(run.env);
            let mut call = run.fetch;
            call.push(invoke(docker, &run.args));
            // The token leaves the session however docker ends, also on Ctrl+C or an error.
            script.push(run_agent(label, &call, &run.cleanup));
        }
    }
    wrap(
        &title,
        &cwd,
        &script.join("; "),
        &resolved.powershell,
        tab(settings, resolved),
        false,
    )
}

/// One `try` around `call` (the agent's command last; statements before it, like the gh
/// token fetch, share the `try`), its exit code taken right after that command. `finally`
/// runs `cleanup`, then closes the terminal after a normal end: one of [`NORMAL_EXIT`], or no
/// code because Ctrl+C stopped the script (statements after the call never run then, a
/// `finally` does). A terminating error of the call (e.g. the CLI cannot start) is rethrown
/// and stays visible; any other code leaves the terminal open with a German line naming it.
fn run_agent(label: &str, call: &[String], cleanup: &[String]) -> String {
    let mut finally = cleanup.to_vec();
    finally.push(format!(
        "if (-not $tgFailed -and ($null -eq $tgCode -or $tgCode -in {NORMAL_EXIT})) {{ [Environment]::Exit(0) }}"
    ));
    format!(
        "$tgCode=$null; $tgFailed=$false; \
         try {{ {}; $tgCode=$LASTEXITCODE }} catch {{ $tgFailed=$true; throw }} \
         finally {{ {} }}; \
         Write-Host ''; Write-Host ({} + $tgCode + {})",
        call.join("; "),
        finally.join("; "),
        ps_literal(&format!("[traygent] {label} wurde mit Code ")),
        ps_literal(" beendet."),
    )
}

/// Points the CLI at its profile: the recipe's variable set to the profile's directory, or —
/// for the default profile of an agent that has extra ones (`clear`) — removed, so a stray
/// value of the terminal's environment cannot move the default home. An agent without
/// profiles, or without an extra one, gets no line.
fn profile_env(agent: AgentId, resolved: &Resolved, clear: bool) -> Result<Option<String>, String> {
    let Some(recipe) = crate::profiles::recipe(agent) else {
        return Ok(None);
    };
    let name = recipe.env_var;
    match &resolved.profile_dir {
        Some(dir) if !is_windows_absolute(dir) => {
            Err("Der Profilordner ist kein absoluter Windows-Pfad.".into())
        }
        Some(dir) => Ok(Some(format!("$env:{name}={}", ps_literal(dir)))),
        None if clear => Ok(Some(format!(
            "Remove-Item Env:{name} -ErrorAction SilentlyContinue"
        ))),
        None => Ok(None),
    }
}

/// Opens a terminal in `home` that signs the agent's profile in: the CLI by absolute path
/// with the recipe's login arguments, in the profile's environment. Stays open like the
/// install terminal.
pub fn login_plan(
    agent: AgentId,
    profile_label: &str,
    settings: &AppSettings,
    resolved: &Resolved,
    home: &str,
) -> Result<LaunchPlan, String> {
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
    let title = format!("{label} · {profile_label} anmelden");
    let args: Vec<String> = recipe
        .login_args
        .iter()
        .map(|arg| arg.to_string())
        .collect();
    let mut script = vec![format!("$Host.UI.RawUI.WindowTitle={}", ps_literal(&title))];
    script.extend(profile_env(agent, resolved, true)?);
    script.push(invoke(&cli.path, &args));
    wrap(
        &title,
        home,
        &script.join("; "),
        &resolved.powershell,
        tab(settings, resolved),
        false,
    )
}

/// `git clone -- <url> <folder>` by absolute path, then a stop on a non-zero exit (also when
/// git could not run: `$LASTEXITCODE` is then unset). Git uses the user's own credentials;
/// no token is involved.
fn clone(
    request: &LaunchRequest,
    folder: &str,
    resolved: &Resolved,
) -> Result<[String; 2], String> {
    let repo = request.repo.as_ref().ok_or(NO_REPO)?;
    let repo = crate::repo::validate(repo)?;
    let git = resolved.git.as_deref().ok_or(GIT_MISSING)?;
    if !is_windows_absolute(git) {
        return Err("Der Pfad zu Git ist kein absoluter Windows-Pfad.".into());
    }
    let args = ["clone", "--", &crate::repo::clone_url(&repo), folder].map(String::from);
    Ok([
        invoke(git, &args),
        format!(
            "if ($LASTEXITCODE -ne 0) {{ throw {} }}",
            ps_literal(CLONE_FAILED)
        ),
    ])
}

/// The request's ports, normalized; only a Docker session publishes any (WSL 2 forwards
/// `localhost` by itself, a local CLI binds on the host anyway).
pub fn request_ports(request: &LaunchRequest) -> Result<Vec<String>, String> {
    let ports = ports::normalize_ports(&request.ports)?;
    if !ports.is_empty() && request.env != EnvId::Docker {
        return Err(ports::DOCKER_ONLY.into());
    }
    Ok(ports)
}

/// Opens a terminal in `home` that runs the catalog install command (explicit click only).
pub fn install_plan(
    agent: AgentId,
    settings: &AppSettings,
    resolved: &Resolved,
    home: &str,
) -> Result<LaunchPlan, String> {
    let title = format!("{} installieren", agent.label());
    // `$?` is the install command's own result: false after an error, a failed script or a
    // non-zero exit code, which `$LASTEXITCODE` also catches for a native installer.
    let script = [
        format!("$Host.UI.RawUI.WindowTitle={}", ps_literal(&title)),
        "$LASTEXITCODE=0".into(),
        catalog::entry(agent).install.command,
        format!(
            "if ($? -and $LASTEXITCODE -eq 0) {{ Write-Host ''; Write-Host {} }} else {{ Write-Host ''; Write-Host {} }}",
            ps_literal(INSTALL_DONE),
            ps_literal(INSTALL_FAILED)
        ),
    ]
    .join("; ");
    // Bypass (process scope): the installers and npm's `.ps1` shim need it under `Restricted`.
    wrap(
        &title,
        home,
        &script,
        &resolved.powershell,
        tab(settings, resolved),
        true,
    )
}

pub const INSTALL_DONE: &str = "[traygent] Fertig. Danach in Traygent die Erkennung aktualisieren.";
pub const INSTALL_FAILED: &str =
    "[traygent] Installation fehlgeschlagen. Bitte die Meldungen oben prüfen.";

/// wt.exe splits its arguments at `;` and may expand `%VAR%`; the tab title gets neither.
fn wt_title(title: &str) -> String {
    title.replace(';', ",").replace('%', "\u{FF05}")
}

/// A Windows Terminal tab: wt.exe and whether it opens in the [`SHARED_WINDOW`].
#[derive(Clone, Copy, Debug)]
struct Tab<'a> {
    wt: &'a str,
    shared: bool,
}

/// A tab when Windows Terminal is chosen and found; otherwise the plan is a console.
fn tab<'a>(settings: &AppSettings, resolved: &'a Resolved) -> Option<Tab<'a>> {
    let wt = resolved.wt.as_deref()?;
    (settings.terminal == TerminalId::WindowsTerminal).then_some(Tab {
        wt,
        shared: settings.shared_window,
    })
}

fn wrap(
    title: &str,
    folder: &str,
    script: &str,
    powershell: &str,
    tab: Option<Tab>,
    bypass: bool,
) -> Result<LaunchPlan, String> {
    let encoded = encode_command(script);
    if encoded.len() > MAX_ENCODED_CHARS {
        return Err(COMMAND_TOO_LONG.into());
    }
    let mut shell = vec!["-NoLogo".to_string(), "-NoExit".into()];
    if bypass {
        shell.extend(["-ExecutionPolicy".into(), "Bypass".into()]);
    }
    shell.extend(["-EncodedCommand".into(), encoded]);
    // powershell.exe sits under %SystemRoot%; should that ever hold `;` or `%`, wt is skipped.
    let tab = tab.filter(|_| !powershell.contains([';', '%']));
    let Some(tab) = tab else {
        return Ok(LaunchPlan {
            program: powershell.into(),
            args: shell,
            cwd: folder.into(),
            title: title.into(),
            terminal: TerminalId::Console,
        });
    };
    let mut args = Vec::new();
    if tab.shared {
        args.extend(["-w".to_string(), SHARED_WINDOW.into()]);
    }
    args.push("new-tab".into());
    // Such a folder relies on the process cwd and Set-Location alone.
    if !folder.contains([';', '%']) {
        args.extend(["-d".into(), folder.into()]);
    }
    // The CLI must not replace the title (OSC sequences, SetConsoleTitle).
    args.extend([
        "--title".into(),
        wt_title(title),
        "--suppressApplicationTitle".into(),
        powershell.into(),
    ]);
    args.extend(shell);
    Ok(LaunchPlan {
        program: tab.wt.into(),
        args,
        cwd: folder.into(),
        title: title.into(),
        terminal: TerminalId::WindowsTerminal,
    })
}

#[cfg(test)]
pub(crate) mod tests;
