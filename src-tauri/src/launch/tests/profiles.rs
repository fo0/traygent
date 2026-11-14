//! The profile's environment line of a local start and both sign-ins (#33, #41).
use super::*;
use crate::models::AgentProfile;

const TEAM_DIR: &str = r"C:\Users\Max O'Neil\.claude-team";
const CLEAR: &str = "Remove-Item Env:CLAUDE_CONFIG_DIR -ErrorAction SilentlyContinue";

fn profiled() -> AppSettings {
    let mut settings = console();
    settings.profiles.claude = vec![AgentProfile {
        id: "team".into(),
        label: "Team".into(),
    }];
    settings
}

fn in_profile(dir: &str) -> Resolved {
    Resolved {
        profile_dir: Some(dir.into()),
        ..resolved(Some((CLAUDE, false)))
    }
}

fn team_request(env: EnvId) -> LaunchRequest {
    LaunchRequest {
        profile: Some("team".into()),
        ..request(AgentId::Claude, env, r"C:\Code\demo")
    }
}

fn statements(plan: &LaunchPlan) -> Vec<String> {
    script(plan).split("; ").map(str::to_string).collect()
}

#[test]
fn a_profile_sets_the_config_dir_as_a_literal_before_the_call() {
    let plan = plan(
        &team_request(EnvId::Local),
        &profiled(),
        &in_profile(TEAM_DIR),
    )
    .unwrap();
    let lines = statements(&plan);
    let env = lines
        .iter()
        .position(|line| line == r"$env:CLAUDE_CONFIG_DIR='C:\Users\Max O''Neil\.claude-team'")
        .expect("the env line, quotes doubled");
    let call = lines.iter().position(|line| line.contains(CLAUDE)).unwrap();
    let location = lines
        .iter()
        .position(|line| line.starts_with("Set-Location"))
        .unwrap();
    assert!(location < env && env < call, "{lines:?}");
    assert!(!script(&plan).contains(CLEAR));
    // Never a relative or unexpanded directory.
    for dir in [r".claude-team", r"%USERPROFILE%\.claude-team", ""] {
        assert_eq!(
            super::plan(&team_request(EnvId::Local), &profiled(), &in_profile(dir)),
            Err("Der Profilordner ist kein absoluter Windows-Pfad.".into()),
            "{dir}"
        );
    }
}

#[test]
fn the_default_profile_removes_a_stray_config_dir_only_when_profiles_exist() {
    let default = request(AgentId::Claude, EnvId::Local, r"C:\Code\demo");
    let cli = resolved(Some((CLAUDE, false)));
    let with = plan(&default, &profiled(), &cli).unwrap();
    assert!(statements(&with).iter().any(|line| line == CLEAR));
    assert!(!script(&with).contains("$env:CLAUDE_CONFIG_DIR"));
    let named = LaunchRequest {
        profile: Some("default".into()),
        ..default.clone()
    };
    assert_eq!(
        script(&plan(&named, &profiled(), &cli).unwrap()),
        script(&with)
    );
    // No extra profile: the script is the one from before the profiles.
    let without = plan(&default, &console(), &cli).unwrap();
    assert!(!script(&without).contains("CLAUDE_CONFIG_DIR"));
    assert_eq!(
        statements(&with)
            .into_iter()
            .filter(|line| line != CLEAR)
            .collect::<Vec<_>>(),
        statements(&without)
    );
    // An agent without a recipe never gets a line.
    let codex = resolved(Some((r"C:\x\codex.exe", false)));
    let mut settings = profiled();
    settings.profiles.codex = settings.profiles.claude.clone();
    let other = plan(
        &request(AgentId::Codex, EnvId::Local, r"C:\Code\demo"),
        &settings,
        &codex,
    );
    assert!(!script(&other.unwrap()).contains("Env:"));
}

#[test]
fn a_request_and_its_resolved_directory_must_agree() {
    let cli = resolved(Some((CLAUDE, false)));
    let unresolved = plan(&team_request(EnvId::Local), &profiled(), &cli);
    assert_eq!(
        unresolved,
        Err("Der Profilordner wurde nicht ermittelt.".into())
    );
    let default = request(AgentId::Claude, EnvId::Local, r"C:\Code\demo");
    let stray = plan(&default, &profiled(), &in_profile(TEAM_DIR));
    assert_eq!(stray, Err("Der Profilordner wurde nicht ermittelt.".into()));
}

#[test]
fn a_docker_start_never_carries_the_profile() {
    let plain = request(AgentId::Claude, EnvId::Docker, r"C:\Code\demo");
    let expected = script(&plan(&plain, &profiled(), &resolved(None)).unwrap());
    // Whatever arrives: the container's own home is used.
    for resolved in [resolved(None), in_profile(TEAM_DIR)] {
        let docker = plan(&team_request(EnvId::Docker), &profiled(), &resolved).unwrap();
        assert_eq!(script(&docker), expected);
        assert!(!script(&docker).contains(".claude-team"));
        assert!(!script(&docker).contains("Env:CLAUDE_CONFIG_DIR"));
    }
}

#[test]
fn the_debug_login_terminal_signs_a_profile_in_and_stays_open() {
    let plan = login_plan(
        AgentId::Claude,
        "Team; %X% 'A'",
        &AppSettings::default(),
        &in_profile(TEAM_DIR),
        r"C:\Users\max",
    )
    .unwrap();
    assert_eq!(plan.program, WT);
    assert_eq!(plan.cwd, r"C:\Users\max");
    assert_eq!(plan.title, "Claude Code · Team; %X% 'A' anmelden");
    // wt.exe gets neither `;` nor `%`.
    let title = plan.args.iter().position(|arg| arg == "--title").unwrap();
    assert_eq!(
        plan.args[title + 1],
        "Claude Code · Team, \u{FF05}X\u{FF05} 'A' anmelden"
    );
    assert_eq!(
        statements(&plan),
        [
            "$Host.UI.RawUI.WindowTitle='Claude Code · Team", // split at the label's own `; `
            "%X% ''A'' anmelden'",
            r"$env:CLAUDE_CONFIG_DIR='C:\Users\Max O''Neil\.claude-team'",
            &format!("& '{CLAUDE}' 'auth' 'login'"),
        ]
    );
    assert!(plan.args.contains(&"-NoExit".to_string()));
    let default = login_plan(
        AgentId::Claude,
        "Standard",
        &console(),
        &resolved(Some((CLAUDE, false))),
        r"C:\Users\max",
    )
    .unwrap();
    assert_eq!(statements(&default)[1], CLEAR);
    assert_eq!(default.terminal, TerminalId::Console);
    let refused = |agent, resolved: &Resolved| {
        login_plan(agent, "Team", &console(), resolved, r"C:\Users\max").unwrap_err()
    };
    assert_eq!(
        refused(AgentId::Claude, &resolved(None)),
        "Claude Code ist nicht installiert."
    );
    assert_eq!(
        refused(AgentId::Claude, &resolved(Some(("claude.exe", false)))),
        "Der Pfad zu Claude Code ist kein absoluter Windows-Pfad."
    );
    assert_eq!(
        refused(AgentId::Opencode, &resolved(Some((CLAUDE, false)))),
        "OpenCode unterstützt keine Profile."
    );
}

#[test]
fn the_hidden_login_runs_the_same_call_and_exits_with_the_clis_code() {
    let hidden =
        hidden_login_plan(AgentId::Claude, &in_profile(TEAM_DIR), r"C:\Users\max").unwrap();
    assert_eq!(hidden.program, PS);
    assert_eq!(hidden.cwd, r"C:\Users\max");
    assert_eq!(
        hidden.args[..4],
        [
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand"
        ]
    );
    assert_eq!(hidden.args.len(), 5);
    // ADR-0003: no `"` and no empty element in the argv; no prompt is kept open.
    assert!(hidden
        .args
        .iter()
        .all(|arg| !arg.is_empty() && !arg.contains('"')));
    assert!(!hidden.args.contains(&"-NoExit".to_string()));
    let script = decode_command(&hidden.args[4]);
    assert_eq!(
        script.split("; ").collect::<Vec<_>>(),
        [
            "$LASTEXITCODE=$null",
            r"$env:CLAUDE_CONFIG_DIR='C:\Users\Max O''Neil\.claude-team'",
            &format!("& '{CLAUDE}' 'auth' 'login'"),
            // A CLI that could not run leaves no code: that is a failure, never a success.
            "if ($null -eq $LASTEXITCODE) { exit 1 }",
            "exit $LASTEXITCODE",
        ]
    );
    // The default profile clears a stray variable; an npm shim is called the same way.
    let shim = r"C:\Users\max\AppData\Roaming\npm\claude.cmd";
    let default = hidden_login_plan(
        AgentId::Claude,
        &resolved(Some((shim, true))),
        r"C:\Users\max",
    )
    .unwrap();
    let script = decode_command(&default.args[4]);
    assert!(script.contains(&format!("; {CLEAR}; & '{shim}' 'auth' 'login'; ")));
    let refused = |agent, resolved: &Resolved| {
        hidden_login_plan(agent, resolved, r"C:\Users\max").unwrap_err()
    };
    assert_eq!(
        refused(AgentId::Claude, &resolved(None)),
        "Claude Code ist nicht installiert."
    );
    assert_eq!(
        refused(AgentId::Codex, &resolved(Some((CLAUDE, false)))),
        "Codex unterstützt keine Profile."
    );
    let relative = Resolved {
        powershell: "powershell.exe".into(),
        ..in_profile(TEAM_DIR)
    };
    assert_eq!(
        refused(AgentId::Claude, &relative),
        "Der Pfad zu PowerShell ist kein absoluter Windows-Pfad."
    );
}
