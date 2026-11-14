//! Launch preparation: the request checked against the file system, the host's executables
//! resolved and the terminal plan built. A repository start reuses the folder it brings (a
//! recent's restart) while that exists, else derives its folder from the current clone root and
//! clones first when that folder does not exist (docs/SPEC.md → Repositories).
use super::Service;
use crate::detect::{host, Detection, Fs, Host};
use crate::launch::{self, HiddenLogin, LaunchPlan, Resolved};
use crate::models::{
    AgentId, AgentStatus, EnvId, LaunchRequest, ProbeState, TerminalId, DEFAULT_PROFILE,
};
use crate::profiles::Homes;
use crate::{repo, settings};

const FOLDER_MISSING: &str = "Ordner nicht gefunden.";
const NO_CLONE_ROOT: &str =
    "Kein Klon-Ordner verfügbar. Bitte in den Einstellungen unter GitHub einen Klon-Ordner wählen.";
const CLONE_ROOT_FAILED: &str = "Der Klon-Ordner konnte nicht angelegt werden.";
pub const PROFILE_MISSING: &str = "Profil nicht gefunden.";
const NO_PROFILE_HOME: &str = "Der Profilordner ist nicht verfügbar: kein Benutzerordner gefunden.";
const PROFILE_DIR_FAILED: &str = "Der Profilordner konnte nicht angelegt werden.";

/// A launch ready to spawn; `fallback` is the console plan when wt.exe fails to start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    /// Normalized: the folder (derived for a repository start), options, ports, repo and
    /// profile (none for the default one and outside the local environment).
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
        let profile =
            self.launch_profile(request.agent, request.env, request.profile.as_deref())?;
        let request = LaunchRequest {
            folder: target.folder,
            repo,
            options: settings::normalize_options(request.options.clone()),
            ports: launch::request_ports(request)?,
            profile,
            ..request.clone()
        };
        let clone = target.clone_root.is_some();
        let mut resolved = self.resolve(request.agent, request.env, clone, host, fs, nonce);
        resolved.profile_dir = profile_dir(request.agent, request.profile.as_deref(), host)?;
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

    /// The profile a start uses: none for the default one and for every environment but the
    /// local one (a container has its own home). Any other must be one of the agent's
    /// profiles — a recent whose profile was removed does not start. „Bearbeiten …“ of a recent
    /// stores its profile by the same rule.
    pub(super) fn launch_profile(
        &self,
        agent: AgentId,
        env: EnvId,
        profile: Option<&str>,
    ) -> Result<Option<String>, String> {
        let wanted = profile
            .map(str::trim)
            .filter(|id| !id.is_empty() && *id != DEFAULT_PROFILE);
        let Some(id) = wanted.filter(|_| env == EnvId::Local) else {
            return Ok(None);
        };
        let listed = self.state.settings.profiles.get(agent);
        if !listed.iter().any(|profile| profile.id == id) {
            return Err(PROFILE_MISSING.into());
        }
        Ok(Some(id.to_string()))
    }

    /// The last look before a prepared start spawns: the profile sync ran without the
    /// service lock, and meanwhile an update of the agent may have begun or the profile may
    /// have been removed.
    pub fn confirm_launch(&self, request: &LaunchRequest) -> Result<(), String> {
        if request.env == EnvId::Local && self.agent_updating(request.agent) {
            return Err(format!(
                "{} wird gerade aktualisiert.",
                request.agent.label()
            ));
        }
        self.launch_profile(request.agent, request.env, request.profile.as_deref())
            .map(drop)
    }

    /// The terminal that signs the agent's profile in (`<cli> auth login`), with its console
    /// fallback („Debug“ on, #41). The profile's directory is created, so the CLI finds its
    /// home.
    pub fn prepare_login(
        &self,
        agent: AgentId,
        profile: &str,
        host: &Host,
        fs: &dyn Fs,
    ) -> Result<(LaunchPlan, Option<LaunchPlan>), String> {
        let (label, resolved, home) = self.sign_in_target(agent, profile, host, fs)?;
        let settings = &self.state.settings;
        let plan = launch::login_plan(agent, &label, settings, &resolved, &home)?;
        let fallback = (plan.terminal == TerminalId::WindowsTerminal)
            .then(|| {
                launch::login_plan(
                    agent,
                    &label,
                    settings,
                    &console_only(resolved.clone()),
                    &home,
                )
                .ok()
            })
            .flatten();
        create_profile_dir(&resolved, fs)?;
        Ok((plan, fallback))
    }

    /// The same sign-in without a window („Debug“ off, #41); the directory is created too.
    pub fn prepare_hidden_login(
        &self,
        agent: AgentId,
        profile: &str,
        host: &Host,
        fs: &dyn Fs,
    ) -> Result<HiddenLogin, String> {
        let (_, resolved, home) = self.sign_in_target(agent, profile, host, fs)?;
        let plan = launch::hidden_login_plan(agent, &resolved, &home)?;
        create_profile_dir(&resolved, fs)?;
        Ok(plan)
    }

    /// What both sign-ins check and resolve: the profile's label, the CLI and the profile's
    /// directory, and the user profile the process starts in. Creates nothing.
    fn sign_in_target(
        &self,
        agent: AgentId,
        profile: &str,
        host: &Host,
        fs: &dyn Fs,
    ) -> Result<(String, Resolved, String), String> {
        let name = agent.label();
        if crate::profiles::recipe(agent).is_none() {
            return Err(format!("{name} unterstützt keine Profile."));
        }
        if self.agent_updating(agent) {
            return Err(format!("{name} wird gerade aktualisiert."));
        }
        let listed = self.state.settings.profiles.get(agent);
        let label = match listed.iter().find(|listed| listed.id == profile) {
            Some(listed) => listed.label.clone(),
            None if profile == DEFAULT_PROFILE => crate::models::DEFAULT_PROFILE_LABEL.into(),
            None => return Err(PROFILE_MISSING.into()),
        };
        let mut resolved = self.resolve(agent, EnvId::Local, false, host, fs, "");
        if resolved.cli.is_none() {
            return Err(format!("{name} ist nicht installiert."));
        }
        let id = (profile != DEFAULT_PROFILE).then_some(profile);
        resolved.profile_dir = profile_dir(agent, id, host)?;
        // A sign-in writes the credentials: never into a directory that is someone else's
        // (it would look like the agent's own from then on and be adopted).
        if let Some(dir) = &resolved.profile_dir {
            let default_dir = profile_dir(agent, Some(DEFAULT_PROFILE), host)?.unwrap_or_default();
            let share = self.state.settings.profile_share.get(agent);
            if fs.refuses_sign_in(agent, dir, &default_dir, share) {
                return Err(format!(
                    "Der {}.",
                    crate::profiles::sync::foreign_dir(agent)
                ));
            }
        }
        let home = host
            .user_profile
            .clone()
            .filter(|dir| fs.is_dir(dir))
            .unwrap_or_else(|| "C:\\".into());
        Ok((label, resolved, home))
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
            profile_dir: None,
        }
    }
}

/// The directory of an extra profile (`None`: the default one, the CLI's own home).
fn profile_dir(agent: AgentId, id: Option<&str>, host: &Host) -> Result<Option<String>, String> {
    let Some(id) = id else {
        return Ok(None);
    };
    let dir = Homes::of(host).and_then(|homes| homes.dir(agent, id));
    dir.map(Some).ok_or_else(|| NO_PROFILE_HOME.into())
}

/// The profile's directory, so the CLI finds its home; the default one is the CLI's own.
fn create_profile_dir(resolved: &Resolved, fs: &dyn Fs) -> Result<(), String> {
    match &resolved.profile_dir {
        Some(dir) => fs
            .create_dir_all(dir)
            .map_err(|_| PROFILE_DIR_FAILED.into()),
        None => Ok(()),
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
