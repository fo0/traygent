//! What the terminal shows and when it closes: the glyph title, Windows Terminal's tab
//! options and the exit handling around the agent call (docs/SPEC.md → Launch rules).
use super::*;

const DOCKER: &str = "C:\\Program Files\\Docker\\Docker\\resources\\bin\\docker.exe";
const GH: &str = r"C:\Program Files\GitHub CLI\gh.exe";
const EXIT: &str =
    "if (-not $tgFailed -and ($null -eq $tgCode -or $tgCode -in 0,130,-1073741510)) \
                    { [Environment]::Exit(0) }";

fn cli(agent: AgentId) -> &'static str {
    match agent {
        AgentId::Claude => CLAUDE,
        AgentId::Opencode => r"C:\Users\max\.opencode\bin\opencode.exe",
        AgentId::Codex => r"C:\x\codex.exe",
    }
}

fn shared() -> AppSettings {
    AppSettings {
        shared_window: true,
        ..AppSettings::default()
    }
}

#[test]
fn titles_are_the_agent_glyph_and_the_folder_name() {
    assert_eq!(
        title(AgentId::Claude, r"C:\Code\RepoWatch"),
        "\u{2733} RepoWatch"
    );
    assert_eq!(title(AgentId::Codex, r"C:\"), "\u{276F} C:");
    assert_eq!(title(AgentId::Opencode, r"\\srv\share"), "\u{25A3} share");
    for agent in AgentId::ALL {
        let plan = plan(
            &request(agent, EnvId::Local, r"C:\Code\demo"),
            &AppSettings::default(),
            &resolved(Some((cli(agent), false))),
        )
        .unwrap();
        assert_eq!(plan.title, format!("{} demo", glyph(agent)));
        assert_eq!(plan.args[4], plan.title);
        assert!(script(&plan).starts_with(&format!(
            "$Host.UI.RawUI.WindowTitle='{} demo'; ",
            glyph(agent)
        )));
    }
}

#[test]
fn every_windows_terminal_tab_keeps_its_title() {
    let session = plan(
        &request(AgentId::Codex, EnvId::Docker, r"C:\Code\demo"),
        &AppSettings::default(),
        &resolved(None),
    )
    .unwrap();
    let install = install_plan(
        AgentId::Codex,
        &AppSettings::default(),
        &resolved(None),
        r"C:\Users\max",
    )
    .unwrap();
    for plan in [session, install] {
        let at = plan.args.iter().position(|arg| arg == "--title").unwrap();
        assert_eq!(plan.args[at + 1], plan.title);
        assert_eq!(plan.args[at + 2], "--suppressApplicationTitle");
        assert_eq!(plan.args[at + 3], PS);
    }
}

#[test]
fn a_windows_terminal_launch_opens_a_tab_without_separators() {
    let plan = plan(
        &request(AgentId::Claude, EnvId::Local, r"C:\Code\demo"),
        &AppSettings::default(),
        &resolved(Some((CLAUDE, false))),
    )
    .unwrap();
    assert_eq!(plan.program, WT);
    assert_eq!(plan.terminal, TerminalId::WindowsTerminal);
    assert_eq!(
        plan.args[..10],
        [
            "new-tab",
            "-d",
            r"C:\Code\demo",
            "--title",
            "\u{2733} demo",
            "--suppressApplicationTitle",
            PS,
            "-NoLogo",
            "-NoExit",
            "-EncodedCommand"
        ]
    );
    assert_eq!(plan.args.len(), 11);
    // Neither wt nor PowerShell's parser ever sees a separator, quote or space of the script.
    for arg in &plan.args[6..] {
        assert!(!arg.contains([';', '%', '"', '\'', ' ', '`']), "{arg}");
    }
    assert!(script(&plan).contains("'; Set-Location -LiteralPath 'C:\\Code\\demo'"));
}

#[test]
fn windows_terminal_is_only_used_when_chosen_and_found() {
    let settings = AppSettings::default();
    let mut without_wt = resolved(Some((CLAUDE, false)));
    without_wt.wt = None;
    let plan = plan(
        &request(AgentId::Claude, EnvId::Local, r"C:\x"),
        &settings,
        &without_wt,
    )
    .unwrap();
    assert_eq!(plan.terminal, TerminalId::Console);
    let chosen_console = super::plan(
        &request(AgentId::Claude, EnvId::Local, r"C:\x"),
        &console(),
        &resolved(Some((CLAUDE, false))),
    )
    .unwrap();
    assert_eq!(chosen_console.program, PS);
    let mut odd_shell = resolved(Some((CLAUDE, false)));
    odd_shell.powershell = r"C:\Win;dows\powershell.exe".into();
    let skipped = super::plan(
        &request(AgentId::Claude, EnvId::Local, r"C:\x"),
        &settings,
        &odd_shell,
    )
    .unwrap();
    assert_eq!(skipped.terminal, TerminalId::Console);
}

#[test]
fn the_shared_window_setting_targets_one_named_window() {
    assert!(!AppSettings::default().shared_window);
    let req = request(AgentId::Claude, EnvId::Local, r"C:\Code\demo");
    let found = resolved(Some((CLAUDE, false)));
    let own = plan(&req, &AppSettings::default(), &found).unwrap();
    assert_eq!(own.args[0], "new-tab");
    assert!(!own.args.iter().any(|arg| arg == "-w"));
    let one = plan(&req, &shared(), &found).unwrap();
    assert_eq!(one.args[..4], ["-w", "traygent", "new-tab", "-d"]);
    assert_eq!(one.args[4..], own.args[2..]);
    let install = install_plan(AgentId::Claude, &shared(), &found, r"C:\Users\max").unwrap();
    assert_eq!(install.args[..3], ["-w", SHARED_WINDOW, "new-tab"]);
    // The console ignores it, also when Windows Terminal is chosen but missing.
    let console = plan(
        &req,
        &AppSettings {
            terminal: TerminalId::Console,
            ..shared()
        },
        &found,
    )
    .unwrap();
    assert_eq!(console.program, PS);
    assert_eq!(console.args[..2], ["-NoLogo", "-NoExit"]);
    let without_wt = Resolved { wt: None, ..found };
    let fallback = plan(&req, &shared(), &without_wt).unwrap();
    assert_eq!(fallback.terminal, TerminalId::Console);
    assert!(!fallback.args.iter().any(|arg| arg == "-w"));
}

/// The terminal stays open (-NoExit) for errors before the start; after the agent call a
/// normal end closes it and any other exit code stays with a German line.
#[test]
fn a_local_start_closes_after_a_normal_end_only() {
    for agent in AgentId::ALL {
        let plan = plan(
            &request(agent, EnvId::Local, r"C:\Code\demo"),
            &console(),
            &resolved(Some((cli(agent), false))),
        )
        .unwrap();
        assert_eq!(plan.args[..2], ["-NoLogo", "-NoExit"]);
        let script = script(&plan);
        let (before, rest) = script
            .split_once("$tgCode=$null; $tgFailed=$false; try { ")
            .unwrap();
        assert!(before.contains("-ErrorAction Stop; "));
        let (call, rest) = rest.split_once("; $tgCode=$LASTEXITCODE } ").unwrap();
        assert!(call.starts_with(&format!("& '{}' ", cli(agent))), "{call}");
        assert_eq!(
            rest,
            format!(
                "catch {{ $tgFailed=$true; throw }} finally {{ {EXIT} }}; Write-Host ''; \
                 Write-Host ('[traygent] {} wurde mit Code ' + $tgCode + ' beendet.')",
                agent.label()
            )
        );
        // Only Claude Code would rewrite the title itself.
        assert_eq!(
            before.contains("$env:CLAUDE_CODE_DISABLE_TERMINAL_TITLE='1'; "),
            agent == AgentId::Claude
        );
    }
}

#[test]
fn a_docker_start_drops_the_token_before_it_closes() {
    let mut settings = console();
    settings.docker.gh_token = true;
    let mut found = resolved(None);
    found.gh = Some(GH.into());
    let plan = plan(
        &request(AgentId::Claude, EnvId::Docker, r"C:\Code\demo"),
        &settings,
        &found,
    )
    .unwrap();
    let script = script(&plan);
    let (_, rest) = script
        .split_once("; $tgCode=$null; $tgFailed=$false; try { ")
        .unwrap();
    assert!(rest.starts_with(&format!(
        "$env:GH_TOKEN=(& '{GH}' auth token 2>$null); & '{DOCKER}' 'run' "
    )));
    // gh's exit code is overwritten by docker's before it is read.
    assert!(rest.contains(
        "'--allow-dangerously-skip-permissions'; $tgCode=$LASTEXITCODE } catch { $tgFailed=$true; throw } "
    ));
    assert!(script.ends_with(&format!(
        "finally {{ [Environment]::SetEnvironmentVariable('GH_TOKEN', $null); {EXIT} }}; \
         Write-Host ''; Write-Host ('[traygent] Claude Code wurde mit Code ' + $tgCode + ' beendet.')"
    )));
    assert_eq!(script.matches("finally {").count(), 1);
    // In Docker the variable goes into the container, not into the host session.
    assert!(!script.contains("$env:CLAUDE_CODE_DISABLE_TERMINAL_TITLE"));
    assert!(script.contains("'-e' 'CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1'"));
    let codex = super::plan(
        &request(AgentId::Codex, EnvId::Docker, r"C:\Code\demo"),
        &console(),
        &resolved(None),
    )
    .unwrap();
    let codex = super::script(&codex);
    assert!(!codex.contains("CLAUDE_CODE_DISABLE_TERMINAL_TITLE"));
    assert!(codex.ends_with(&format!(
        "finally {{ {EXIT} }}; Write-Host ''; \
         Write-Host ('[traygent] Codex wurde mit Code ' + $tgCode + ' beendet.')"
    )));
}

#[test]
fn the_install_terminal_stays_open() {
    let plan = install_plan(AgentId::Claude, &console(), &resolved(None), r"C:\x").unwrap();
    let script = script(&plan);
    assert!(!script.contains("Exit"));
    assert!(!script.contains("$tgCode"));
}
