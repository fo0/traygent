//! Launch preparation: the request checked against the file system, the host's executables
//! resolved and the terminal plan built. A repository start reuses the folder it brings (a
//! recent's restart) while that exists, else derives its folder from the current clone root and
//! clones first when that folder does not exist (docs/SPEC.md → Repositories).
use super::Service;
use crate::detect::{host, Detection, Fs, Host};
use crate::launch::{self, LaunchPlan, Resolved};
use crate::models::{AgentId, AgentStatus, EnvId, LaunchRequest, ProbeState, TerminalId};
use crate::{repo, settings};

const FOLDER_MISSING: &str = "Ordner nicht gefunden.";
const NO_CLONE_ROOT: &str =
    "Kein Klon-Ordner verfügbar. Bitte in den Einstellungen unter GitHub einen Klon-Ordner wählen.";
const CLONE_ROOT_FAILED: &str = "Der Klon-Ordner konnte nicht angelegt werden.";

/// A launch ready to spawn; `fallback` is the console plan when wt.exe fails to start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    /// Normalized: the folder (derived for a repository start), options, ports and repo.
    pub request: LaunchRequest,
    pub plan: LaunchPlan,
    pub fallback: Option<LaunchPlan>,
}

/// Where a start runs: the folder, and the clone root when the folder must be cloned first.
struct Target {
    folder: String,
    clone_root: Option<String>,
}

impl Service {
    /// Validates the request against the file system and builds the terminal plan. A
    /// repository start whose folder is missing creates the clone root (only once the plan is
    /// built) and clones in the session's own script.
    pub fn prepare_launch(
        &self,
        request: &LaunchRequest,
        host: &Host,
        fs: &dyn Fs,
        nonce: &str,
    ) -> Result<Prepared, String> {
        // The update replaces the local CLI; a session started now could find it half written.
        if request.env == EnvId::Local && self.agent_updating(request.agent) {
            return Err(format!(
                "{} wird gerade aktualisiert.",
                request.agent.label()
            ));
        }
        let repo = request.repo.as_ref().map(repo::validate).transpose()?;
        let target = match &repo {
            Some(repo) => self.repo_target(repo, &request.folder, host, fs)?,
            None => {
                let folder = launch::normalize_folder(&request.folder)?;
                if !fs.is_dir(&folder) {
                    return Err(FOLDER_MISSING.into());
                }
                Target {
                    folder,
                    clone_root: None,
                }
            }
        };
        let request = LaunchRequest {
            folder: target.folder,
            repo,
            options: settings::normalize_options(request.options.clone()),
            ports: launch::request_ports(request)?,
            ..request.clone()
        };
        let clone = target.clone_root.is_some();
        let resolved = self.resolve(request.agent, request.env, clone, host, fs, nonce);
        let settings = &self.state.settings;
        let build = |resolved: &Resolved| match &target.clone_root {
            Some(root) => launch::clone_plan(&request, settings, resolved, root),
            None => launch::plan(&request, settings, resolved),
        };
        let plan = build(&resolved)?;
        let fallback = (plan.terminal == TerminalId::WindowsTerminal)
            .then(|| build(&console_only(resolved)).ok())
            .flatten();
        if let Some(root) = &target.clone_root {
            fs.create_dir_all(root).map_err(|_| CLONE_ROOT_FAILED)?;
        }
        Ok(Prepared {
            request,
            plan,
            fallback,
        })
    }

    /// The given folder while it exists, so a recent's clone stays in use after the clone
    /// root changed (#40); else `<clone root>\<owner>\<name>`, reused when it is a folder,
    /// else cloned into.
    fn repo_target(
        &self,
        repo: &crate::models::RepoRef,
        folder: &str,
        host: &Host,
        fs: &dyn Fs,
    ) -> Result<Target, String> {
        if !folder.trim().is_empty() {
            let folder = launch::normalize_folder(folder)?;
            if fs.is_dir(&folder) {
                return Ok(Target {
                    folder,
                    clone_root: None,
                });
            }
        }
        let configured = &self.state.settings.github.clone_root;
        let root = if configured.is_empty() {
            repo::default_clone_root(host)
        } else {
            launch::normalize_folder(configured)?
        };
        if root.is_empty() {
            return Err(NO_CLONE_ROOT.into());
        }
        let folder = repo::clone_folder(&root, repo)?;
        let clone_root = (!fs.is_dir(&folder)).then_some(root);
        Ok(Target { folder, clone_root })
    }

    /// The install terminal for `agent`, with its console fallback.
    pub fn prepare_install(
        &self,
        agent: AgentId,
        host: &Host,
        fs: &dyn Fs,
    ) -> Result<(LaunchPlan, Option<LaunchPlan>), String> {
        let resolved = self.resolve(agent, EnvId::Local, false, host, fs, "");
        let home = host
            .user_profile
            .clone()
            .filter(|dir| fs.is_dir(dir))
            .unwrap_or_else(|| "C:\\".into());
        let settings = &self.state.settings;
        let plan = launch::install_plan(agent, settings, &resolved, &home)?;
        let fallback = (plan.terminal == TerminalId::WindowsTerminal)
            .then(|| launch::install_plan(agent, settings, &console_only(resolved), &home).ok())
            .flatten();
        Ok((plan, fallback))
    }

    fn resolve(
        &self,
        agent: AgentId,
        env: EnvId,
        clone: bool,
        host: &Host,
        fs: &dyn Fs,
        nonce: &str,
    ) -> Resolved {
        let settings = &self.state.settings;
        let detection = self.detection.as_ref();
        let exists = |path: &String| fs.is_file(path);
        let cli = (env == EnvId::Local).then(|| {
            // A detection's choice (e.g. the V2 OpenCode among several) wins while it exists.
            let detected = detection.and_then(|found| found.clis.get(agent).clone());
            let rejected = detection
                .and_then(|found| local_status(found, agent))
                .filter(|status| status.state != ProbeState::Ready)
                .and_then(|status| status.path.clone());
            detected.filter(|cli| exists(&cli.path)).or_else(|| {
                host::agent_candidates(agent, host)
                    .into_iter()
                    .find(|cli| exists(&cli.path) && Some(&cli.path) != rejected.as_ref())
            })
        });
        let docker = (env == EnvId::Docker).then(|| {
            let detected = detection.and_then(|found| found.docker.clone());
            detected
                .filter(exists)
                .or_else(|| host::first(fs, host::docker_candidates(host)))
        });
        let gh = (env == EnvId::Docker && settings.docker.gh_token)
            .then(|| host::first(fs, host::gh_candidates(host)));
        let gitconfig = (env == EnvId::Docker)
            .then(|| host.gitconfig().filter(exists))
            .flatten();
        let git = clone.then(|| host::first(fs, host::git_candidates(host)));
        let wt = (settings.terminal == TerminalId::WindowsTerminal)
            .then(|| host::first(fs, host::wt_candidates(host)));
        Resolved {
            cli: cli.flatten(),
            docker: docker.flatten(),
            gh: gh.flatten(),
            gitconfig,
            git: git.flatten(),
            powershell: host.powershell(),
            wt: wt.flatten(),
            nonce: nonce.into(),
        }
    }
}

fn console_only(resolved: Resolved) -> Resolved {
    Resolved {
        wt: None,
        ..resolved
    }
}

fn local_status(detection: &Detection, agent: AgentId) -> Option<&AgentStatus> {
    detection
        .agents
        .iter()
        .find(|status| status.agent == agent && status.env == EnvId::Local)
}
