//! Detection: which agent CLIs are installed locally and whether Docker can run them.
//!
//! Pure orchestration over [`Fs`] and [`Runner`]; every probe runs on its own thread, so a
//! full detection takes about one probe timeout (two when Docker is ready).
use crate::launch::docker::tools_volume;
use crate::models::{AgentId, AgentStatus, EnvId, EnvStatus, PerAgent, ProbeState};
use crate::settings;
use chrono::{DateTime, Utc};

pub mod host;
pub mod runner;

pub use host::{Candidate, Fs, Host, RealFs};
pub use runner::{RunError, RunOutput, Runner, SystemRunner};

/// At most this many existing candidates get a `--version` probe (OpenCode V1 collisions).
const MAX_PROBES: usize = 3;
pub const SOON: &str = "Bald verfügbar";
const NO_VERSION: &str = "Version nicht ermittelbar";
const OPENCODE_V1_NPM: &str =
    "OpenCode 1 gefunden – zuerst „npm uninstall -g opencode-ai“ ausführen, dann V2 installieren";
const OPENCODE_V1: &str = "OpenCode 1 gefunden – alte Installation entfernen, dann V2 installieren";
const NOT_STARTABLE: &str = "CLI lässt sich nicht starten";
const GIT_RECOMMENDED: &str = "Git for Windows empfohlen";
const DOCKER_MISSING: &str = "Docker nicht gefunden";
const DOCKER_STOPPED: &str = "Docker läuft nicht";
const DOCKER_SILENT: &str = "Docker antwortet nicht";
const DOCKER_WINDOWS: &str = "Docker nutzt Windows-Container – auf Linux-Container umschalten";
const IMAGE_INVALID: &str = "Docker-Image ist ungültig";
const FIRST_INSTALL: &str = "CLI wird beim ersten Start installiert";
const FIRST_PULL: &str = "Image wird beim ersten Start geladen";
const FIRST_BOTH: &str = "Image und CLI werden beim ersten Start geladen";

/// One finished detection: the panel's statuses plus the resolved executables for launching.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detection {
    pub agents: Vec<AgentStatus>,
    pub environments: Vec<EnvStatus>,
    /// The local CLI each agent launches with, when one is ready.
    pub clis: PerAgent<Option<Candidate>>,
    pub docker: Option<String>,
    pub checked_at: String,
}

/// `\d+\.\d+\.\d+`, first match.
pub fn parse_version(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let digits_from = |mut at: usize| {
        let start = at;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            at += 1;
        }
        (at > start).then_some(at)
    };
    for start in 0..bytes.len() {
        if !bytes[start].is_ascii_digit() || (start > 0 && bytes[start - 1].is_ascii_digit()) {
            continue;
        }
        let mut end = start;
        let mut groups = 0;
        while let Some(after) = digits_from(end) {
            groups += 1;
            end = after;
            if groups == 3 {
                return Some(text[start..end].to_string());
            }
            if bytes.get(end) != Some(&b'.') {
                break;
            }
            end += 1;
        }
    }
    None
}

fn major(version: &str) -> Option<u64> {
    version.split('.').next()?.parse().ok()
}

pub fn status(agent: AgentId, env: EnvId, state: ProbeState) -> AgentStatus {
    AgentStatus {
        agent,
        env,
        state,
        version: None,
        path: None,
        detail: None,
    }
}

fn with_detail(mut status: AgentStatus, detail: Option<&str>) -> AgentStatus {
    status.detail = detail.map(Into::into);
    status
}

/// Resolves the agent's CLI and asks it for its version.
pub fn probe_local(
    agent: AgentId,
    host: &Host,
    fs: &dyn Fs,
    runner: &dyn Runner,
) -> (AgentStatus, Option<Candidate>) {
    let found: Vec<Candidate> = host::agent_candidates(agent, host)
        .into_iter()
        .filter(|candidate| fs.is_file(&candidate.path))
        .collect();
    if found.is_empty() {
        return (status(agent, EnvId::Local, ProbeState::Missing), None);
    }
    let mut outdated: Option<(Candidate, String)> = None;
    for candidate in found.iter().take(MAX_PROBES) {
        let version = match runner.run(&candidate.path, &["--version"]) {
            Ok(output) => parse_version(&output.stdout),
            Err(RunError::Missing) => continue,
            Err(_) => None,
        };
        if agent == AgentId::Opencode {
            if let Some(old) = version.as_ref().filter(|v| major(v).is_some_and(|m| m < 2)) {
                outdated.get_or_insert((candidate.clone(), old.clone()));
                continue;
            }
        }
        let mut ready = status(agent, EnvId::Local, ProbeState::Ready);
        ready.path = Some(candidate.path.clone());
        ready.detail = if version.is_none() {
            Some(NO_VERSION.into())
        } else if agent == AgentId::Claude && host.windows {
            host::first(fs, host::git_candidates(host))
                .is_none()
                .then(|| GIT_RECOMMENDED.into())
        } else {
            None
        };
        ready.version = version;
        return (ready, Some(candidate.clone()));
    }
    let state = match outdated {
        Some((candidate, version)) => AgentStatus {
            version: Some(version),
            // V1's npm package `opencode-ai` owns the `opencode.cmd` V2 needs (npm EEXIST).
            detail: Some(
                if candidate.shim {
                    OPENCODE_V1_NPM
                } else {
                    OPENCODE_V1
                }
                .into(),
            ),
            path: Some(candidate.path),
            ..status(agent, EnvId::Local, ProbeState::Missing)
        },
        None => with_detail(
            status(agent, EnvId::Local, ProbeState::Unavailable),
            Some(NOT_STARTABLE),
        ),
    };
    (state, None)
}

fn env_status(env: EnvId, state: ProbeState, detail: Option<&str>) -> EnvStatus {
    EnvStatus {
        env,
        state,
        version: None,
        detail: detail.map(Into::into),
    }
}

/// The Docker engine: CLI present, daemon answering, Linux containers.
pub fn probe_engine(host: &Host, fs: &dyn Fs, runner: &dyn Runner) -> (EnvStatus, Option<String>) {
    let Some(docker) = host::first(fs, host::docker_candidates(host)) else {
        let missing = env_status(EnvId::Docker, ProbeState::Missing, Some(DOCKER_MISSING));
        return (missing, None);
    };
    let format = "{{.Server.Version}}/{{.Server.Os}}";
    let engine = match runner.run(&docker, &["version", "--format", format]) {
        Ok(output) if output.success => {
            let line = output.stdout.trim();
            let (version, os) = line.split_once('/').unwrap_or((line, ""));
            if version.trim().is_empty() {
                env_status(EnvId::Docker, ProbeState::Stopped, Some(DOCKER_STOPPED))
            } else if os.trim().eq_ignore_ascii_case("linux") {
                EnvStatus {
                    version: Some(version.trim().to_string()),
                    ..env_status(EnvId::Docker, ProbeState::Ready, None)
                }
            } else {
                EnvStatus {
                    version: Some(version.trim().to_string()),
                    ..env_status(EnvId::Docker, ProbeState::Unavailable, Some(DOCKER_WINDOWS))
                }
            }
        }
        Err(RunError::Missing) => {
            env_status(EnvId::Docker, ProbeState::Missing, Some(DOCKER_MISSING))
        }
        Err(RunError::Timeout) => {
            env_status(EnvId::Docker, ProbeState::Stopped, Some(DOCKER_SILENT))
        }
        _ => env_status(EnvId::Docker, ProbeState::Stopped, Some(DOCKER_STOPPED)),
    };
    (engine, Some(docker))
}

fn exists(runner: &dyn Runner, docker: &str, kind: &str, name: &str) -> bool {
    runner
        .run(docker, &[kind, "inspect", name])
        .is_ok_and(|output| output.success)
}

/// Per-agent Docker readiness: the engine's state; when ready, a first-start note when the
/// image or the CLI volume does not exist yet.
pub fn probe_docker_agents(
    engine: &EnvStatus,
    docker: Option<&str>,
    runner: &dyn Runner,
    images: &PerAgent<String>,
) -> Vec<AgentStatus> {
    let (ProbeState::Ready, Some(docker)) = (engine.state, docker) else {
        return AgentId::ALL
            .into_iter()
            .map(|agent| {
                let state = status(agent, EnvId::Docker, engine.state);
                with_detail(state, engine.detail.as_deref())
            })
            .collect();
    };
    std::thread::scope(|scope| {
        let probes: Vec<_> = AgentId::ALL
            .into_iter()
            .map(|agent| {
                let image = images.get(agent).as_str();
                // Never hand a value that could read as a flag to `docker image inspect`.
                let valid = settings::is_valid_image(image);
                let volume =
                    scope.spawn(move || exists(runner, docker, "volume", tools_volume(agent)));
                let pulled =
                    valid.then(|| scope.spawn(move || exists(runner, docker, "image", image)));
                (agent, volume, pulled)
            })
            .collect();
        probes
            .into_iter()
            .map(|(agent, volume, pulled)| {
                let volume = volume.join().unwrap_or(true);
                let Some(pulled) = pulled else {
                    let invalid = status(agent, EnvId::Docker, ProbeState::Unavailable);
                    return with_detail(invalid, Some(IMAGE_INVALID));
                };
                let pulled = pulled.join().unwrap_or(true);
                let detail = match (pulled, volume) {
                    (true, true) => None,
                    (true, false) => Some(FIRST_INSTALL),
                    (false, true) => Some(FIRST_PULL),
                    (false, false) => Some(FIRST_BOTH),
                };
                with_detail(status(agent, EnvId::Docker, ProbeState::Ready), detail)
            })
            .collect()
    })
}

/// Runs every probe in parallel and assembles the statuses in contract order.
pub fn detect(
    host: &Host,
    fs: &dyn Fs,
    runner: &dyn Runner,
    images: &PerAgent<String>,
    now: DateTime<Utc>,
) -> Detection {
    let (locals, (engine, docker, docker_agents)) = std::thread::scope(|scope| {
        let locals: Vec<_> = AgentId::ALL
            .into_iter()
            .map(|agent| scope.spawn(move || probe_local(agent, host, fs, runner)))
            .collect();
        let docker = scope.spawn(|| {
            let (engine, docker) = probe_engine(host, fs, runner);
            let agents = probe_docker_agents(&engine, docker.as_deref(), runner, images);
            (engine, docker, agents)
        });
        let locals: Vec<_> = AgentId::ALL
            .into_iter()
            .zip(locals)
            .map(|(agent, probe)| {
                probe
                    .join()
                    .unwrap_or_else(|_| (status(agent, EnvId::Local, ProbeState::Unknown), None))
            })
            .collect();
        let docker = docker.join().unwrap_or_else(|_| {
            let unknown = env_status(EnvId::Docker, ProbeState::Unknown, None);
            let agents = probe_docker_agents(&unknown, None, runner, images);
            (unknown, None, agents)
        });
        (locals, docker)
    });
    let mut clis = PerAgent::default();
    let mut agents = Vec::new();
    for ((agent, (local, cli)), in_docker) in
        AgentId::ALL.into_iter().zip(locals).zip(docker_agents)
    {
        *clis.get_mut(agent) = cli;
        agents.push(local);
        agents.push(in_docker);
    }
    Detection {
        agents,
        environments: vec![
            env_status(EnvId::Local, ProbeState::Ready, None),
            engine,
            env_status(EnvId::Wsl, ProbeState::Unavailable, Some(SOON)),
        ],
        clis,
        docker,
        checked_at: crate::recents::timestamp(now),
    }
}

#[cfg(test)]
mod tests;
