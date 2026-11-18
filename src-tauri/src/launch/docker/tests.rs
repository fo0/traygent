use super::*;
use crate::launch::plan;
use crate::launch::tests::{env_value, request, resolved, script, PS};
use crate::models::{EnvId, PermissionId, SessionMode, TerminalId};

mod socket;

#[test]
fn a_codex_shim_must_run_before_it_can_skip_installation() {
    let init = init_script(AgentId::Codex, false);
    let (before, after) = init
        .split_once("npm install -g --include=optional @openai/codex@latest || exit 1")
        .unwrap();
    assert!(before.contains("if ! as_codex --version"));
    assert!(before.contains("as_codex() { runuser -u node -- /home/node/.local/bin/codex"));
    assert!(after.contains("codex_cli=(\"$(command -v node)\" /home/node/.local/bin/codex)"));
    let (after_install, _) = after.split_once("if ! codex_login status").unwrap();
    assert!(after_install.contains("as_codex --version ||"));
}

const DOCKER: &str = "C:\\Program Files\\Docker\\Docker\\resources\\bin\\docker.exe";

fn console() -> AppSettings {
    AppSettings {
        terminal: TerminalId::Console,
        ..AppSettings::default()
    }
}

/// The `docker` argv of the script's final call, unquoted.
fn docker_args(script: &str) -> Vec<String> {
    let call = format!("& '{DOCKER}' ");
    let start = script.find(&call).unwrap() + call.len();
    let line = script[start..]
        .split("; $tgCode=$LASTEXITCODE")
        .next()
        .unwrap();
    let mut args = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('\'') {
        let mut close = open + 1;
        let mut value = String::new();
        loop {
            let next = rest[close..].find('\'').unwrap() + close;
            value.push_str(&rest[close..next]);
            if rest[next + 1..].starts_with('\'') {
                value.push('\'');
                close = next + 2;
            } else {
                close = next + 1;
                break;
            }
        }
        args.push(value);
        rest = &rest[close..];
    }
    args
}

#[test]
fn project_dir_names_match_the_starter() {
    assert_eq!(
        project_dir_name(r"C:\Users\Max\My Project\"),
        "my-project-f7b09a93"
    );
    assert_eq!(project_dir_name(r"C:\Code\RepoWatch"), "repowatch-7a494d81");
    assert_eq!(
        project_dir_name(r"D:\Ünïcode Ordner"),
        "--n--code-ordner-66394083"
    );
    // Whitespace runs collapse like the fragment's unquoted echo; case never matters.
    assert_eq!(
        project_dir_name(r"C:\Users\Max\My   Project"),
        "my-project-f7b09a93"
    );
    assert_eq!(
        project_dir_name(r"c:\users\max\my project"),
        "my-project-f7b09a93"
    );
    let long = project_dir_name(&format!(r"C:\{}", "x".repeat(60)));
    assert_eq!(long.len(), 40 + 1 + 8);
}

#[test]
fn init_scripts_are_lf_only_and_end_in_the_cli() {
    for (agent, last) in [
        (
            AgentId::Claude,
            "exec runuser -u node -- env PATH=\"$agent_path\" /home/node/.local/bin/claude \"$@\"\n",
        ),
        (
            AgentId::Opencode,
            "exec runuser -u node -- env PATH=\"$agent_path\" /home/node/.opencode/bin/opencode --standalone \"$@\"\n",
        ),
        (
            AgentId::Codex,
            "exec runuser -u node -- env PATH=\"$agent_path\" \"${codex_cli[@]}\" -c \"sqlite_home=$CODEX_SQLITE_HOME\" \"$@\"\n",
        ),
    ] {
        let script = init_script(agent, false);
        assert!(!script.contains('\r'));
        assert!(script.ends_with(last), "{agent:?}");
    }
    for agent in [AgentId::Claude, AgentId::Codex] {
        let init = init_script(agent, false);
        assert!(!init.contains("rm -rf"));
        for chown in init.lines().filter(|line| line.starts_with("chown -R ")) {
            assert!(!chown.contains("/.claude") && !chown.contains("/.codex"));
        }
    }
}

#[cfg(unix)]
#[test]
fn init_scripts_and_loader_are_valid_bash() {
    if std::process::Command::new("bash")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    for script in AgentId::ALL
        .into_iter()
        .flat_map(|agent| [init_script(agent, false), init_script(agent, true)])
        .chain(
            [
                LOADER,
                include_str!("../../../resources/docker/install-gh.sh"),
                include_str!("../../../resources/docker/prepare-node"),
                include_str!("../../../resources/docker/node-hook"),
            ]
            .map(String::from),
        )
    {
        let status = std::process::Command::new("bash")
            .args(["-n", "-c", &script])
            .status()
            .unwrap();
        assert!(status.success(), "{script}");
    }
}

#[test]
fn claude_mounts_the_host_config_and_state_and_reuses_the_tools_and_project_dir() {
    let mut req = request(AgentId::Claude, EnvId::Docker, r"C:\Code\RepoWatch");
    req.options.permission = PermissionId::Bypass;
    req.options.model = "opus[1m]".into();
    req.session = SessionMode::Continue;
    let plan = plan(&req, &console(), &resolved(None)).unwrap();
    assert_eq!(plan.program, PS);
    let text = script(&plan);
    assert_eq!(
        env_value(&text, "TRAYGENT_INIT").unwrap(),
        init_script(AgentId::Claude, false)
    );
    let expected = [
        "run",
        "--rm",
        "-it",
        "--init",
        "--security-opt",
        "no-new-privileges",
        "--name",
        "traygent-claude-repowatch-7a494d81-a1b2c3",
        "-v",
        r"C:\Code\RepoWatch:/workspace/repowatch-7a494d81",
        "-w",
        "/workspace/repowatch-7a494d81",
        "-v",
        r"C:\Program Files\Traygent\docker:/opt/traygent:ro",
        "-e",
        "TRAYGENT_MANAGE_NODE=1",
        "-e",
        "TRAYGENT_HOOK_NODES",
        "-v",
        r"C:\Agent Home:/home/node/.claude:rw",
        "-v",
        r"C:\Users\Max\.claude.json:/home/node/.claude/.claude.json:rw",
        "-v",
        "claude-code-tools:/home/node/.local",
        "-e",
        "HOME=/home/node",
        "-e",
        "CLAUDE_CONFIG_DIR=/home/node/.claude",
        "-e",
        "ANTHROPIC_API_KEY",
        "-e",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "-e",
        "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        "-e",
        "CLAUDE_CODE_PROJECT_DIR_NAME=repowatch-7a494d81",
        "-e",
        "CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1",
        "-e",
        "GIT_CONFIG_COUNT=2",
        "-e",
        "GIT_CONFIG_KEY_0=safe.directory",
        "-e",
        "GIT_CONFIG_VALUE_0=*",
        "-e",
        "GIT_CONFIG_KEY_1=core.autocrlf",
        "-e",
        "GIT_CONFIG_VALUE_1=input",
        "-e",
        "TRAYGENT_INIT",
        "-e",
        "TERM=xterm-256color",
        "-e",
        "COLORTERM=truecolor",
        "node:22-trixie",
        "bash",
        "-c",
        LOADER,
        "traygent",
        "--dangerously-skip-permissions",
        "--model",
        "opus[1m]",
        "--continue",
    ];
    assert_eq!(docker_args(&text), expected);
    // The trust prompt stays: skipping it would let a repo's hooks run unasked (S2).
    assert!(!text.contains("TRUST_WORKSPACE"));
}

#[test]
fn opencode_mounts_per_project_and_forwards_its_config_name_only() {
    let mut req = request(AgentId::Opencode, EnvId::Docker, r"C:\Users\Max\My Project");
    req.options.model = "opencode/big-pickle".into();
    req.options.permission = PermissionId::Auto;
    let plan = plan(&req, &console(), &resolved(None)).unwrap();
    let text = script(&plan);
    let config = env_value(&text, "OPENCODE_CONFIG_CONTENT").unwrap();
    assert_eq!(config, r#"{"model":"opencode/big-pickle"}"#);
    let args = docker_args(&text);
    let joined = args.join(" ");
    assert!(joined.starts_with("run --rm -it --init --security-opt no-new-privileges --name traygent-opencode-my-project-f7b09a93-a1b2c3"));
    assert!(joined.contains(r"-v C:\Users\Max\My Project:/workspace/my-project-f7b09a93 -w /workspace/my-project-f7b09a93"));
    for volume in [
        "traygent-opencode-bin:/home/node/.opencode",
        "traygent-opencode-data:/home/node/.local",
        "traygent-opencode-config:/home/node/.config/opencode",
    ] {
        assert!(args.contains(&volume.to_string()), "{volume}");
    }
    assert!(joined.contains("-e OPENCODE_CONFIG_CONTENT -e GIT_CONFIG_COUNT=2"));
    assert!(joined.ends_with("node:22-trixie bash -c printenv TRAYGENT_INIT >/tmp/traygent-init.sh && . /tmp/traygent-init.sh traygent --auto"));
    let plain = super::super::plan(
        &request(AgentId::Opencode, EnvId::Docker, r"C:\x"),
        &console(),
        &resolved(None),
    )
    .unwrap();
    assert!(!docker_args(&script(&plain)).contains(&"OPENCODE_CONFIG_CONTENT".to_string()));
}

#[test]
fn codex_mounts_the_host_home_and_uses_the_container_permission_mapping() {
    let mut req = request(AgentId::Codex, EnvId::Docker, r"C:\Code\RepoWatch");
    req.options.permission = PermissionId::Ask;
    req.session = SessionMode::Resume;
    let mut settings = console();
    *settings.docker.images.get_mut(AgentId::Codex) = "ghcr.io/me/codex:1".into();
    let args = docker_args(&script(&plan(&req, &settings, &resolved(None)).unwrap()));
    let joined = args.join(" ");
    assert!(joined.starts_with("run --rm -it --init --security-opt no-new-privileges --name "));
    assert!(joined.contains(
        r"-v traygent-codex-tools:/home/node/.local -v C:\Agent Home:/home/node/.codex:rw -e CODEX_HOME=/home/node/.codex"
    ));
    assert!(joined.contains("-e OPENAI_API_KEY"));
    assert!(joined.contains("--tmpfs /home/node/.codex/tmp:rw,exec,nosuid,nodev,mode=0700"));
    assert!(init_script(AgentId::Codex, false)
        .contains("chown node:node /home/node/.codex/tmp || exit 1"));
    assert!(joined.contains("-e CODEX_SQLITE_HOME=/home/node/.local/share/traygent-codex/sqlite"));
    assert!(joined.contains(
        "ghcr.io/me/codex:1 bash -c printenv TRAYGENT_INIT >/tmp/traygent-init.sh && . /tmp/traygent-init.sh traygent resume --sandbox danger-full-access --ask-for-approval on-request"
    ));
}

#[test]
fn codex_hands_only_a_windows_created_installation_id_to_node_before_the_tui() {
    let init = init_script(AgentId::Codex, false);
    let lines: Vec<&str> = init.lines().collect();
    let at = |needle: &str| lines.iter().position(|line| line.contains(needle)).unwrap();
    let guard = at("codex_id=/home/node/.codex/installation_id");
    let change_owner = at("chown -h node:node \"$codex_id\"");
    assert!(guard < at("[ ! -L \"$codex_id\" ]"));
    assert!(at("stat -c %h") < change_owner && at("id -u node") < change_owner);
    assert!(lines[change_owner].ends_with("exit 1; }"));
    assert!(guard < at("codex_login status") && guard < at("exec runuser"));
    // The only host file whose ownership view changes; nothing recursive under .codex.
    let host_chowns: Vec<&&str> = lines
        .iter()
        .filter(|line| line.contains("chown") && !line.contains("/home/node/.local"))
        .collect();
    assert_eq!(host_chowns.len(), 2, "{host_chowns:?}");
    assert!(!init.contains("chown -R node:node /home/node/.codex"));
    // Codex sets the mode itself once node owns the file; the init never widens it.
    assert!(!lines.iter().any(|line| line.contains("chmod")));
}

#[test]
fn the_host_gitconfig_is_mounted_read_only_only_when_it_exists() {
    let req = request(AgentId::Claude, EnvId::Docker, r"C:\x");
    let without = docker_args(&script(&plan(&req, &console(), &resolved(None)).unwrap()));
    assert!(!without.iter().any(|arg| arg.contains(".gitconfig")));
    let mut found = resolved(None);
    found.gitconfig = Some(r"C:\Users\Max Mustermann\.gitconfig".into());
    let with = docker_args(&script(&plan(&req, &console(), &found).unwrap()));
    assert!(
        with.contains(&r"C:\Users\Max Mustermann\.gitconfig:/home/node/.gitconfig:ro".to_string())
    );
}

#[test]
fn the_selected_account_token_is_fetched_in_the_session_and_forwarded_name_only() {
    let req = request(AgentId::Codex, EnvId::Docker, r"C:\x");
    let mut settings = console();
    settings.docker.gh_token = true;
    let mut found = resolved(None);
    let text = script(&plan(&req, &settings, &found).unwrap());
    assert!(text.contains(r"; try { $env:GH_TOKEN=(& 'C:\Program Files\Traygent\traygent.exe' '--docker-github-token' '42' 2>$null | Out-String).Trim(); if ($LASTEXITCODE -ne 0"));
    assert!(text.contains("finally { [Environment]::SetEnvironmentVariable('GH_TOKEN', $null)"));
    let args = docker_args(&text);
    let joined = args.join(" ");
    assert!(joined.contains("-e GIT_CONFIG_COUNT=4"));
    assert!(joined.contains(
        "-e GIT_CONFIG_KEY_2=credential.https://github.com.helper -e GIT_CONFIG_VALUE_2="
    ));
    assert!(joined.contains(&format!("-e GIT_CONFIG_KEY_3=credential.https://github.com.helper -e GIT_CONFIG_VALUE_3={GIT_HELPER} -e GH_TOKEN")));
    assert!(!args.iter().any(|arg| arg.starts_with("GH_TOKEN=")));
    assert!(!text.contains("gh.exe"));
    found.github_account_id = None;
    assert_eq!(
        plan(&req, &settings, &found).unwrap_err(),
        crate::github::docker::NO_ACCOUNT
    );
    let off = plan(&req, &console(), &found).unwrap();
    assert!(!script(&off).contains("GH_TOKEN"));
}

#[test]
fn docker_rejects_network_paths_and_needs_the_cli() {
    let settings = console();
    for folder in [r"\\server\share\repo", r"\\wsl$\Ubuntu\home\max"] {
        let error = plan(
            &request(AgentId::Claude, EnvId::Docker, folder),
            &settings,
            &resolved(None),
        )
        .unwrap_err();
        assert!(
            error.starts_with("Docker kann keine Netzwerk- oder WSL-Pfade einbinden."),
            "{folder}"
        );
    }
    let mut no_docker = resolved(None);
    no_docker.docker = None;
    assert_eq!(
        plan(
            &request(AgentId::Codex, EnvId::Docker, r"C:\x"),
            &settings,
            &no_docker
        ),
        Err("Docker ist nicht installiert.".into())
    );
}

#[test]
fn a_drive_root_mounts_without_a_trailing_backslash_problem() {
    let args = docker_args(&script(
        &plan(
            &request(AgentId::Claude, EnvId::Docker, r"C:\"),
            &console(),
            &resolved(None),
        )
        .unwrap(),
    ));
    assert!(args.contains(&format!(r"C:\:/workspace/{}", project_dir_name(r"C:\"))));
    assert!(args.contains(&format!(
        "CLAUDE_CODE_PROJECT_DIR_NAME={}",
        project_dir_name(r"C:\")
    )));
}

/// A tampered image in `state.json` would land right before `bash` as a flag (S1).
#[test]
fn a_tampered_image_never_reaches_docker() {
    let req = request(AgentId::Claude, EnvId::Docker, r"C:\x");
    for image in [
        "--privileged",
        "-eX=\u{A0}\" \"--privileged",
        "node:22 --privileged",
        "",
    ] {
        let mut settings = console();
        *settings.docker.images.get_mut(AgentId::Claude) = image.into();
        assert_eq!(
            plan(&req, &settings, &resolved(None)),
            Err("Docker-Image für Claude Code ist ungültig.".into()),
            "{image}"
        );
        let direct = run(
            AgentId::Claude,
            r"C:\x",
            vec![],
            None,
            HostAccess::default(),
            &settings,
            &resolved(None),
        );
        assert!(direct.is_err(), "{image}");
    }
}

#[test]
fn ports_become_p_flags_right_before_the_image_for_every_agent() {
    for agent in AgentId::ALL {
        let mut req = request(agent, EnvId::Docker, r"C:\Code\demo");
        req.ports = vec![
            "3000".into(),
            " 0.0.0.0:8080:80/udp ".into(),
            "".into(),
            "9000-9001:4000-4001".into(),
        ];
        let plan = plan(&req, &console(), &resolved(None)).unwrap();
        let args = docker_args(&script(&plan));
        let image = args.iter().position(|arg| arg == "node:22-trixie").unwrap();
        assert_eq!(
            args[image - 6..image],
            [
                "-p",
                "127.0.0.1:3000:3000",
                "-p",
                "0.0.0.0:8080:80/udp",
                "-p",
                "127.0.0.1:9000-9001:4000-4001"
            ],
            "{agent:?}"
        );
        assert_eq!(args.iter().filter(|arg| *arg == "-p").count(), 3);
        assert_eq!(args[image + 1..image + 3], ["bash", "-c"]);
        for arg in plan.args.iter().chain(&args) {
            assert!(!arg.is_empty() && !arg.contains('"'), "{arg}");
        }
    }
}

#[test]
fn no_ports_publish_nothing() {
    for agent in AgentId::ALL {
        let req = request(agent, EnvId::Docker, r"C:\Code\demo");
        let args = docker_args(&script(&plan(&req, &console(), &resolved(None)).unwrap()));
        assert!(!args.iter().any(|arg| arg == "-p"), "{agent:?}");
    }
}

#[test]
fn invalid_or_non_docker_ports_refuse_the_launch() {
    let cases = [
        (
            "3000 --privileged",
            "Port „3000 --privileged“ ist ungültig.",
        ),
        ("-p", "Port „-p“ ist ungültig."),
        ("3000;calc", "Port „3000;calc“ ist ungültig."),
        ("70000", "Port „70000“ liegt außerhalb von 1–65535."),
    ];
    for (spec, error) in cases {
        let mut req = request(AgentId::Claude, EnvId::Docker, r"C:\Code\demo");
        req.ports = vec![spec.into()];
        assert_eq!(
            plan(&req, &console(), &resolved(None)),
            Err(error.into()),
            "{spec}"
        );
    }
    let mut twice = request(AgentId::Codex, EnvId::Docker, r"C:\Code\demo");
    twice.ports = vec!["3000".into(), "0.0.0.0:3000:80".into()];
    assert_eq!(
        plan(&twice, &console(), &resolved(None)),
        Err("Host-Port 3000 ist doppelt belegt.".into())
    );
    let mut local = request(AgentId::Claude, EnvId::Local, r"C:\Code\demo");
    local.ports = vec!["3000".into()];
    let cli = Some((r"C:\Users\max\.local\bin\claude.exe", false));
    assert_eq!(
        plan(&local, &console(), &resolved(cli)),
        Err("Ports lassen sich nur für Docker-Sessions freigeben.".into())
    );
    // Blank entries are no ports, so a local start still works.
    local.ports = vec![" ".into(), "".into()];
    assert!(plan(&local, &console(), &resolved(cli)).is_ok());
}

#[test]
fn port_flags_keep_windows_terminal_args_free_of_separators() {
    let mut req = request(AgentId::Opencode, EnvId::Docker, r"C:\Code\demo");
    req.ports = vec!["0.0.0.0:5173:5173".into(), "53/udp".into()];
    let plan = plan(&req, &AppSettings::default(), &resolved(None)).unwrap();
    assert_eq!(plan.terminal, TerminalId::WindowsTerminal);
    for arg in &plan.args {
        assert!(!arg.is_empty() && !arg.contains(['"', ';', '%']), "{arg}");
    }
    let script = script(&plan);
    assert!(script.contains("'-p' '0.0.0.0:5173:5173' '-p' '127.0.0.1:53:53/udp' 'node:22-trixie'"));
}
