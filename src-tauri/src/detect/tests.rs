use super::host::{self, dedup, expand_vars, split_path};
use super::*;
use crate::testing::{windows_host, FakeFs, NPM};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Replies keyed by `<program> <args…>`; anything else fails like a crashing process.
#[derive(Default)]
struct FakeRunner {
    replies: HashMap<String, Result<RunOutput, RunError>>,
    delay: Option<Duration>,
    calls: Mutex<Vec<String>>,
}

impl FakeRunner {
    fn reply(mut self, call: &str, stdout: &str) -> Self {
        let output = RunOutput {
            success: true,
            stdout: stdout.into(),
        };
        self.replies.insert(call.into(), Ok(output));
        self
    }
    fn fail(mut self, call: &str, error: Result<RunOutput, RunError>) -> Self {
        self.replies.insert(call.into(), error);
        self
    }
}

impl Runner for FakeRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<RunOutput, RunError> {
        if let Some(delay) = self.delay {
            std::thread::sleep(delay);
        }
        let call = format!("{program} {}", args.join(" "));
        self.calls.lock().unwrap().push(call.clone());
        self.replies
            .get(&call)
            .cloned()
            .unwrap_or(Err(RunError::Failed))
    }
}

const CLAUDE_EXE: &str = r"C:\Users\max\.local\bin\claude.exe";
const DOCKER_EXE: &str = r"C:\Program Files\Docker\Docker\resources\bin\docker.exe";
const DOCKER_VERSION: &str = "version --format {{.Server.Version}}/{{.Server.Os}}";

fn paths(candidates: Vec<Candidate>) -> Vec<String> {
    candidates.into_iter().map(|c| c.path).collect()
}

#[test]
fn versions_are_the_first_three_part_number() {
    let cases = [
        ("2.1.317 (Claude Code)", Some("2.1.317")),
        ("opencode v2.2.5", Some("2.2.5")),
        ("codex-cli 0.166.0\n", Some("0.166.0")),
        ("1.4.0", Some("1.4.0")),
        ("v10.20.30.40", Some("10.20.30")),
        ("12.3.x then 4.5.6", Some("4.5.6")),
        ("1.2", None),
        ("", None),
        ("no digits", None),
    ];
    for (text, expected) in cases {
        assert_eq!(parse_version(text).as_deref(), expected, "{text}");
    }
}

#[test]
fn path_values_split_expand_and_dedup() {
    assert_eq!(
        split_path(r#"C:\a; ;"C:\b c";"#, true),
        [r"C:\a", r"C:\b c"]
    );
    assert_eq!(split_path("/usr/bin:/bin", false), ["/usr/bin", "/bin"]);
    let lookup = |name: &str| (name == "USERPROFILE").then(|| r"C:\Users\max".to_string());
    assert_eq!(
        expand_vars(r"%USERPROFILE%\bin;%NOPE%\x;100%;%%", lookup),
        r"C:\Users\max\bin;%NOPE%\x;100%;%%"
    );
    let dirs = vec![r"C:\A".into(), r"c:\a\".into(), r"C:\B".into()];
    assert_eq!(dedup(dirs, true), [r"C:\A", r"C:\B"]);
}

/// A relative or unexpanded PATH entry could plant a CLI next to Traygent's cwd (S4).
#[test]
fn relative_and_unexpanded_path_entries_are_never_searched() {
    let mut host = windows_host();
    host.path_dirs = vec![
        ".".into(),
        "bin".into(),
        r"%NOPE%\bin".into(),
        r"C:\tools;x".into(),
        r"C:\tools".into(),
        r"\\server\share\bin".into(),
    ];
    let list = paths(host::agent_candidates(AgentId::Codex, &host));
    assert!(
        list.iter()
            .all(|path| path.starts_with(r"C:\") || path.starts_with(r"\\")),
        "{list:?}"
    );
    assert!(list.contains(&r"C:\tools\codex.exe".to_string()));
    assert!(list.contains(&r"\\server\share\bin\codex.exe".to_string()));
    assert!(!list.iter().any(|path| path.contains('%')));
    let opencode = paths(host::agent_candidates(AgentId::Opencode, &host));
    assert!(!opencode
        .iter()
        .any(|path| path.starts_with("bin") || path.starts_with('.')));
    assert!(host::is_search_dir("/usr/bin", false));
    assert!(!host::is_search_dir("usr/bin", false));
    assert!(!host::is_search_dir(r"C:\%X%", true));
}

#[test]
fn claude_candidates_put_native_exes_before_npm_shims() {
    let list = host::agent_candidates(AgentId::Claude, &windows_host());
    let shims: Vec<_> = list.iter().map(|c| c.shim).collect();
    let first_shim = shims.iter().position(|shim| *shim).unwrap();
    assert!(shims[first_shim..].iter().all(|shim| *shim));
    assert!(paths(list.clone()).contains(&CLAUDE_EXE.to_string()));
    assert_eq!(list.last().unwrap().path, format!(r"{NPM}\claude.cmd"));
    // The npm dir is on PATH and is also the known location: listed once.
    let npm_shim = format!(r"{NPM}\claude.cmd");
    assert_eq!(list.iter().filter(|c| c.path == npm_shim).count(), 1);
}

#[test]
fn opencode_candidates_resolve_the_npm_binary_and_prefer_opencode2() {
    let list = paths(host::agent_candidates(AgentId::Opencode, &windows_host()));
    let real = format!(r"{NPM}\node_modules\@opencode\cli\bin\opencode.exe");
    let position = |path: &str| list.iter().position(|p| p == path).unwrap();
    assert!(position(&real) < position(&format!(r"{NPM}\opencode2.cmd")));
    assert!(position(&format!(r"{NPM}\opencode2.cmd")) < position(&format!(r"{NPM}\opencode.cmd")));
    assert!(list.contains(&r"C:\Users\max\.opencode\bin\opencode.exe".to_string()));
}

#[test]
fn codex_candidates_include_the_native_installer_dir() {
    let mut host = windows_host();
    host.codex_install_dir = Some(r"D:\tools\codex".into());
    let list = paths(host::agent_candidates(AgentId::Codex, &host));
    assert!(list.contains(&r"D:\tools\codex\codex.exe".to_string()));
    let native = r"C:\Users\max\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe";
    assert!(list.contains(&native.to_string()));
}

/// A relative env var (`CODEX_INSTALL_DIR=tools`) never yields a candidate (S4).
#[test]
fn relative_env_dirs_yield_no_candidates() {
    let mut host = Host {
        path_dirs: Vec::new(),
        user_profile: Some("max".into()),
        app_data: Some(r"AppData\Roaming".into()),
        local_app_data: Some(r".\Local".into()),
        program_files: Some("Program Files".into()),
        system_root: Some(r"%SystemRoot%".into()),
        codex_install_dir: Some("tools".into()),
        ..windows_host()
    };
    for agent in [AgentId::Claude, AgentId::Opencode, AgentId::Codex] {
        assert!(host::agent_candidates(agent, &host).is_empty(), "{agent:?}");
    }
    assert!(host::docker_candidates(&host).is_empty());
    assert!(host::gh_candidates(&host).is_empty() && host::git_candidates(&host).is_empty());
    assert!(host::wt_candidates(&host).is_empty() && host.gitconfig().is_none());
    assert_eq!(host.powershell(), "powershell.exe");
    assert_eq!(host.taskkill(), r"C:\Windows\System32\taskkill.exe");
    host.codex_install_dir = Some(r"D:\codex".into());
    assert_eq!(
        paths(host::agent_candidates(AgentId::Codex, &host))[0],
        r"D:\codex\codex.exe"
    );
}

#[test]
fn tool_candidates_cover_the_known_locations() {
    let host = windows_host();
    assert!(host::docker_candidates(&host).contains(&DOCKER_EXE.to_string()));
    let rancher = r"C:\Users\max\.rd\bin\docker.exe".to_string();
    assert!(host::docker_candidates(&host).contains(&rancher));
    let wt = host::wt_candidates(&host);
    assert_eq!(
        wt[0],
        r"C:\Users\max\AppData\Local\Microsoft\WindowsApps\wt.exe"
    );
    assert!(host::gh_candidates(&host).contains(&r"C:\Program Files\GitHub CLI\gh.exe".to_string()));
    assert_eq!(
        host.powershell(),
        r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
    );
    let moved = Host {
        system_root: Some(r"D:\Win\".into()),
        ..windows_host()
    };
    assert_eq!(moved.taskkill(), r"D:\Win\System32\taskkill.exe");
    assert_eq!(
        host.gitconfig().as_deref(),
        Some(r"C:\Users\max\.gitconfig")
    );
    let linux = Host {
        path_dirs: vec!["/usr/bin".into()],
        ..Host::default()
    };
    assert_eq!(host::docker_candidates(&linux), ["/usr/bin/docker"]);
    assert!(host::wt_candidates(&linux).is_empty());
}

#[test]
fn a_ready_claude_reports_version_path_and_the_git_hint() {
    let fs = FakeFs::with(&[CLAUDE_EXE]);
    let runner =
        FakeRunner::default().reply(&format!("{CLAUDE_EXE} --version"), "2.1.317 (Claude Code)");
    let (status, cli) = probe_local(AgentId::Claude, &windows_host(), &fs, &runner);
    assert_eq!(status.state, ProbeState::Ready);
    assert_eq!(status.version.as_deref(), Some("2.1.317"));
    assert_eq!(status.path.as_deref(), Some(CLAUDE_EXE));
    assert_eq!(status.detail.as_deref(), Some("Git for Windows empfohlen"));
    assert!(!cli.unwrap().shim);
    let fs = FakeFs::with(&[CLAUDE_EXE, r"C:\Program Files\Git\cmd\git.exe"]);
    let (status, _) = probe_local(AgentId::Claude, &windows_host(), &fs, &runner);
    assert_eq!(status.detail, None);
}

#[test]
fn an_exe_wins_over_an_npm_shim() {
    let shim = format!(r"{NPM}\claude.cmd");
    let fs = FakeFs::with(&[&shim, CLAUDE_EXE]);
    let runner = FakeRunner::default()
        .reply(&format!("{CLAUDE_EXE} --version"), "2.1.317")
        .reply(&format!("{shim} --version"), "2.0.0");
    let (status, cli) = probe_local(AgentId::Claude, &windows_host(), &fs, &runner);
    assert_eq!(status.path.as_deref(), Some(CLAUDE_EXE));
    assert_eq!(runner.calls.lock().unwrap().len(), 1);
    let fs = FakeFs::with(&[&shim]);
    let (_, cli_shim) = probe_local(AgentId::Claude, &windows_host(), &fs, &runner);
    assert!(cli_shim.unwrap().shim);
    assert!(!cli.unwrap().shim);
}

#[test]
fn missing_and_silent_clis() {
    let host = windows_host();
    let (status, cli) = probe_local(
        AgentId::Codex,
        &host,
        &FakeFs::default(),
        &FakeRunner::default(),
    );
    assert_eq!((status.state, cli), (ProbeState::Missing, None));
    let codex = r"C:\Users\max\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe";
    let fs = FakeFs::with(&[codex]);
    let runner = FakeRunner::default().fail(&format!("{codex} --version"), Err(RunError::Timeout));
    let (status, cli) = probe_local(AgentId::Codex, &host, &fs, &runner);
    assert_eq!(status.state, ProbeState::Ready);
    assert_eq!(status.version, None);
    assert_eq!(status.detail.as_deref(), Some("Version nicht ermittelbar"));
    assert!(cli.is_some());
    let runner = FakeRunner::default().fail(&format!("{codex} --version"), Err(RunError::Missing));
    let (status, cli) = probe_local(AgentId::Codex, &host, &fs, &runner);
    assert_eq!((status.state, cli), (ProbeState::Unavailable, None));
}

#[test]
fn opencode_needs_major_version_two() {
    let host = windows_host();
    let v1 = format!(r"{NPM}\opencode.cmd");
    let fs = FakeFs::with(&[&v1]);
    let runner = FakeRunner::default().reply(&format!("{v1} --version"), "1.4.0");
    let (status, cli) = probe_local(AgentId::Opencode, &host, &fs, &runner);
    assert_eq!(status.state, ProbeState::Missing);
    assert_eq!(status.version.as_deref(), Some("1.4.0"));
    // The detail says how to fix it (F3): V1's npm package owns the `opencode.cmd`.
    assert_eq!(
        status.detail.as_deref(),
        Some("OpenCode 1 gefunden – zuerst „npm uninstall -g opencode-ai“ ausführen, dann V2 installieren")
    );
    assert_eq!(cli, None);
    let standalone = r"C:\Users\max\.opencode\bin\opencode.exe";
    let runner_v1 = FakeRunner::default().reply(&format!("{standalone} --version"), "1.2.0");
    let (status, _) = probe_local(
        AgentId::Opencode,
        &host,
        &FakeFs::with(&[standalone]),
        &runner_v1,
    );
    assert_eq!(status.path.as_deref(), Some(standalone));
    assert_eq!(
        status.detail.as_deref(),
        Some("OpenCode 1 gefunden – alte Installation entfernen, dann V2 installieren")
    );
    let old_exe = r"C:\Users\max\.opencode\bin\opencode.exe";
    let v2 = format!(r"{NPM}\opencode2.cmd");
    let fs = FakeFs::with(&[old_exe, &v2, &v1]);
    let runner = FakeRunner::default()
        .reply(&format!("{old_exe} --version"), "1.0.5")
        .reply(&format!("{v2} --version"), "opencode v2.2.5");
    let (status, cli) = probe_local(AgentId::Opencode, &host, &fs, &runner);
    assert_eq!(status.state, ProbeState::Ready);
    assert_eq!(status.version.as_deref(), Some("2.2.5"));
    assert_eq!(
        cli,
        Some(Candidate {
            path: v2,
            shim: true
        })
    );
}

fn engine(runner: &FakeRunner, files: &[&str]) -> (EnvStatus, Option<String>) {
    probe_engine(&windows_host(), &FakeFs::with(files), runner)
}

#[test]
fn the_docker_engine_states() {
    let call = format!("{DOCKER_EXE} {DOCKER_VERSION}");
    let (status, docker) = engine(&FakeRunner::default(), &[]);
    assert_eq!((status.state, docker), (ProbeState::Missing, None));
    assert_eq!(status.detail.as_deref(), Some("Docker nicht gefunden"));
    let (status, docker) = engine(
        &FakeRunner::default().reply(&call, "27.5.1/linux\n"),
        &[DOCKER_EXE],
    );
    assert_eq!(status.state, ProbeState::Ready);
    assert_eq!(status.version.as_deref(), Some("27.5.1"));
    assert_eq!(docker.as_deref(), Some(DOCKER_EXE));
    let (status, _) = engine(
        &FakeRunner::default().reply(&call, "27.5.1/windows"),
        &[DOCKER_EXE],
    );
    assert_eq!(status.state, ProbeState::Unavailable);
    assert!(status.detail.unwrap().contains("Windows-Container"));
    let down = Ok(RunOutput {
        success: false,
        stdout: "/".into(),
    });
    let (status, _) = engine(&FakeRunner::default().fail(&call, down), &[DOCKER_EXE]);
    assert_eq!(
        (status.state, status.detail.as_deref()),
        (ProbeState::Stopped, Some("Docker läuft nicht"))
    );
    let (status, _) = engine(&FakeRunner::default().reply(&call, "/"), &[DOCKER_EXE]);
    assert_eq!(status.state, ProbeState::Stopped);
    let slow = FakeRunner::default().fail(&call, Err(RunError::Timeout));
    let (status, _) = engine(&slow, &[DOCKER_EXE]);
    assert_eq!(
        (status.state, status.detail.as_deref()),
        (ProbeState::Stopped, Some("Docker antwortet nicht"))
    );
}

#[test]
fn docker_agents_note_what_the_first_start_downloads() {
    let images = PerAgent::from_fn(|agent| match agent {
        AgentId::Codex => "custom:1".to_string(),
        _ => "node:22-trixie".to_string(),
    });
    let ready = EnvStatus {
        env: EnvId::Docker,
        state: ProbeState::Ready,
        version: Some("27.5.1".into()),
        detail: None,
    };
    let runner = FakeRunner::default()
        .reply("docker volume inspect claude-code-tools", "[]")
        .reply("docker image inspect node:22-trixie", "[]")
        .reply("docker volume inspect traygent-codex-tools", "[]");
    let statuses = probe_docker_agents(&ready, Some("docker"), &runner, &images);
    let details: Vec<_> = statuses.iter().map(|s| s.detail.as_deref()).collect();
    assert_eq!(
        details,
        [
            None,
            Some("CLI wird beim ersten Start installiert"),
            Some("Image wird beim ersten Start geladen")
        ]
    );
    assert!(statuses
        .iter()
        .all(|s| s.state == ProbeState::Ready && s.env == EnvId::Docker));
    let none = probe_docker_agents(&ready, Some("docker"), &FakeRunner::default(), &images);
    assert_eq!(
        none[0].detail.as_deref(),
        Some("Image und CLI werden beim ersten Start geladen")
    );
    let stopped = EnvStatus {
        state: ProbeState::Stopped,
        detail: Some("Docker läuft nicht".into()),
        ..ready
    };
    let statuses = probe_docker_agents(&stopped, Some("docker"), &FakeRunner::default(), &images);
    assert!(statuses.iter().all(|s| s.state == ProbeState::Stopped));
    assert_eq!(statuses[2].detail.as_deref(), Some("Docker läuft nicht"));
}

/// An image that could read as a flag never reaches `docker image inspect` (S1).
#[test]
fn an_invalid_image_is_never_inspected() {
    let images = PerAgent::from_fn(|agent| match agent {
        AgentId::Claude => "--privileged".to_string(),
        _ => "node:22-trixie".to_string(),
    });
    let ready = EnvStatus {
        env: EnvId::Docker,
        state: ProbeState::Ready,
        version: Some("27.5.1".into()),
        detail: None,
    };
    let runner = FakeRunner::default();
    let statuses = probe_docker_agents(&ready, Some("docker"), &runner, &images);
    assert_eq!(statuses[0].state, ProbeState::Unavailable);
    assert_eq!(
        statuses[0].detail.as_deref(),
        Some("Docker-Image ist ungültig")
    );
    let calls = runner.calls.lock().unwrap();
    assert!(
        !calls.iter().any(|call| call.contains("--privileged")),
        "{calls:?}"
    );
    assert!(calls.contains(&"docker image inspect node:22-trixie".to_string()));
}

#[test]
fn detect_assembles_statuses_in_contract_order_in_about_one_timeout() {
    let images = PerAgent::from_fn(|_| "node:22-trixie".to_string());
    let mut runner = FakeRunner::default()
        .reply(&format!("{CLAUDE_EXE} --version"), "2.1.317")
        .reply(&format!("{DOCKER_EXE} {DOCKER_VERSION}"), "27.5.1/linux");
    runner.delay = Some(Duration::from_millis(400));
    let fs = FakeFs::with(&[CLAUDE_EXE, DOCKER_EXE]);
    let now = chrono::DateTime::parse_from_rfc3339("2026-11-03T09:30:00Z")
        .unwrap()
        .to_utc();
    let started = Instant::now();
    let found = detect(&windows_host(), &fs, &runner, &images, now);
    // Sequentially these 8 probes would take 3.2 s; in parallel the version, then the inspects.
    assert!(
        started.elapsed() < Duration::from_millis(2000),
        "{:?}",
        started.elapsed()
    );
    let order: Vec<_> = found.agents.iter().map(|s| (s.agent, s.env)).collect();
    let expected: Vec<_> = AgentId::ALL
        .into_iter()
        .flat_map(|agent| [(agent, EnvId::Local), (agent, EnvId::Docker)])
        .collect();
    assert_eq!(order, expected);
    assert_eq!(found.agents[0].state, ProbeState::Ready);
    assert_eq!(found.agents[2].state, ProbeState::Missing);
    let envs: Vec<_> = found
        .environments
        .iter()
        .map(|e| (e.env, e.state))
        .collect();
    assert_eq!(
        envs,
        [
            (EnvId::Local, ProbeState::Ready),
            (EnvId::Docker, ProbeState::Ready),
            (EnvId::Wsl, ProbeState::Unavailable)
        ]
    );
    assert_eq!(
        found.environments[2].detail.as_deref(),
        Some("Bald verfügbar")
    );
    assert_eq!(found.checked_at, "2026-11-03T09:30:00Z");
    assert_eq!(found.docker.as_deref(), Some(DOCKER_EXE));
    assert_eq!(
        found.clis.claude.as_ref().map(|c| c.path.as_str()),
        Some(CLAUDE_EXE)
    );
    assert!(found.clis.codex.is_none());
}

#[test]
fn read_capped_keeps_the_head_and_drains_the_rest() {
    let data = vec![7u8; 100_000];
    assert_eq!(runner::read_capped(Some(&data[..]), 10), vec![7u8; 10]);
    assert_eq!(runner::read_capped(None::<&[u8]>, 10), Vec::<u8>::new());
}

#[cfg(unix)]
mod system_runner {
    use super::*;

    fn runner(millis: u64) -> SystemRunner {
        SystemRunner {
            timeout: Duration::from_millis(millis),
            ..SystemRunner::default()
        }
    }

    #[test]
    fn reads_stdout_and_the_exit_status() {
        let output = runner(5000)
            .run("sh", &["-c", "echo 'tool v1.2.3'; echo noise >&2"])
            .unwrap();
        assert!(output.success);
        assert_eq!(parse_version(&output.stdout).as_deref(), Some("1.2.3"));
        assert!(!output.stdout.contains("noise"));
        let failed = runner(5000).run("sh", &["-c", "exit 3"]).unwrap();
        assert!(!failed.success);
    }

    #[test]
    fn stdin_is_closed() {
        let started = Instant::now();
        assert!(runner(5000).run("cat", &[]).unwrap().success);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn times_out_and_kills() {
        let started = Instant::now();
        assert_eq!(runner(200).run("sleep", &["5"]), Err(RunError::Timeout));
        // The tree kill is Windows-only; elsewhere the child is killed alone, as before.
        let tree = SystemRunner {
            kill_tree: true,
            ..runner(200)
        };
        assert_eq!(tree.run("sleep", &["5"]), Err(RunError::Timeout));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn caps_the_output() {
        let output = runner(5000)
            .run("head", &["-c", "200000", "/dev/zero"])
            .unwrap();
        assert_eq!(output.stdout.len(), runner::MAX_OUTPUT);
    }

    #[test]
    fn a_missing_program_is_missing() {
        assert_eq!(
            runner(1000).run("/nonexistent/traygent-tool", &[]),
            Err(RunError::Missing)
        );
    }
}
