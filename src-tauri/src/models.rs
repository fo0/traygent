//! The IPC data model, mirrored field by field from `src/lib/types.ts` (camelCase JSON).
//!
//! `src/lib/types.ts` is the contract; every enum here serializes to exactly the TS string
//! union, every optional TS field is an `Option` that is skipped when `None`.
use serde::{Deserialize, Serialize};

mod github;
pub use github::*;

/// The coding agents Traygent starts.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum AgentId {
    #[default]
    Claude,
    Opencode,
    Codex,
}

impl AgentId {
    pub const ALL: [AgentId; 3] = [Self::Claude, Self::Opencode, Self::Codex];

    /// The contract id, as it appears in JSON.
    pub fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Opencode => "opencode",
            Self::Codex => "codex",
        }
    }

    /// The product name shown in the panel, titles and errors.
    pub fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Opencode => "OpenCode",
            Self::Codex => "Codex",
        }
    }
}

/// Where a session runs; `wsl` is reserved and never ready yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvId {
    #[default]
    Local,
    Docker,
    Wsl,
}

impl EnvId {
    pub const ALL: [EnvId; 3] = [Self::Local, Self::Docker, Self::Wsl];

    pub fn id(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Docker => "docker",
            Self::Wsl => "wsl",
        }
    }
}

/// Unified permission mode; every agent maps it to its own flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionId {
    Ask,
    #[default]
    Auto,
    Bypass,
}

/// Per start only, never a default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionMode {
    #[default]
    New,
    Continue,
    Resume,
}

/// The window a session opens in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalId {
    #[default]
    WindowsTerminal,
    Console,
}

/// Result of a probe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProbeState {
    Ready,
    Missing,
    Stopped,
    Unavailable,
    #[default]
    Unknown,
}

/// One selectable value; `id` `default` sends no flag.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionChoice {
    pub id: PermissionId,
    pub label: String,
    pub hint: String,
    /// Skips every prompt; the panel shows it in the warning colour.
    pub danger: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallInfo {
    /// Best-practice PowerShell one-liner, shown and copyable.
    pub command: String,
    pub docs_url: String,
    /// Update command, when the agent has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<String>,
    /// Prerequisite shown next to the command, e.g. „Node.js 20+“.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<String>,
}

/// Static per-agent capabilities, sent by Rust so the panel renders generically.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCatalogEntry {
    pub id: AgentId,
    pub label: String,
    /// First entry is `default` (no model flag).
    pub models: Vec<Choice>,
    /// Any model string is allowed besides the listed ones.
    pub free_model: bool,
    /// `[]` = no thinking level; `default` sends no flag.
    pub efforts: Vec<Choice>,
    pub permissions: Vec<PermissionChoice>,
    /// Supported session modes; always contains `new`.
    pub sessions: Vec<SessionMode>,
    /// Environments it can run in.
    pub environments: Vec<EnvId>,
    pub install: InstallInfo,
}

/// What a start uses; agent defaults and recent sessions store this.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LaunchOptions {
    pub model: String,
    pub effort: String,
    pub permission: PermissionId,
    /// `''` = none; split on whitespace, validated, each part one literal.
    pub extra_args: String,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            model: "default".into(),
            effort: "default".into(),
            permission: PermissionId::Auto,
            extra_args: String::new(),
        }
    }
}

/// A GitHub repository, `github.com/<owner>/<name>` (docs/adr/0005); validated by
/// [`crate::repo::validate`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRef {
    /// 1–39 of `A-Z a-z 0-9 -`, no leading or trailing `-`.
    pub owner: String,
    /// 1–100 of `A-Z a-z 0-9 . _ -`, not `.`/`..`, without a `.git` suffix.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub agent: AgentId,
    pub env: EnvId,
    /// Absolute Windows folder path. With `repo`: `''` derives `<cloneRoot>\<owner>\<name>`; a
    /// folder (a recent's restart) is reused while it exists, else derived as with `''`.
    pub folder: String,
    /// Start in this repository: an existing clone folder is reused, otherwise the session
    /// script clones it first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<RepoRef>,
    pub options: LaunchOptions,
    pub session: SessionMode,
    /// Published ports for a Docker session (`docker run -p`), normalized specs like
    /// `127.0.0.1:3000:3000`; always `[]` for any other environment (docs/SPEC.md → Ports).
    #[serde(default)]
    pub ports: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentSession {
    /// Stable per (lowercased folder, agent, env).
    pub id: String,
    pub agent: AgentId,
    pub env: EnvId,
    pub folder: String,
    /// The repository the folder was cloned from, when started from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<RepoRef>,
    pub options: LaunchOptions,
    /// The ports the last start published (Docker only, else `[]`).
    #[serde(default)]
    pub ports: Vec<String>,
    /// RFC 3339.
    pub last_started_at: String,
    pub starts: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub agent: AgentId,
    pub env: EnvId,
    pub state: ProbeState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Resolved CLI path (local only), for the tooltip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Short German note, e.g. „CLI wird beim ersten Start installiert“.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvStatus {
    pub env: EnvId,
    pub state: ProbeState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// `Record<AgentId, T>`: one value per agent, keyed by the agent id.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerAgent<T: Default> {
    pub claude: T,
    pub opencode: T,
    pub codex: T,
}

impl<T: Default> PerAgent<T> {
    pub fn from_fn(mut value: impl FnMut(AgentId) -> T) -> Self {
        Self {
            claude: value(AgentId::Claude),
            opencode: value(AgentId::Opencode),
            codex: value(AgentId::Codex),
        }
    }

    pub fn get(&self, agent: AgentId) -> &T {
        match agent {
            AgentId::Claude => &self.claude,
            AgentId::Opencode => &self.opencode,
            AgentId::Codex => &self.codex,
        }
    }

    pub fn get_mut(&mut self, agent: AgentId) -> &mut T {
        match agent {
            AgentId::Claude => &mut self.claude,
            AgentId::Opencode => &mut self.opencode,
            AgentId::Codex => &mut self.codex,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DockerSettings {
    /// Base image per agent.
    pub images: PerAgent<String>,
    /// Forward the host `gh auth token` name-only as GH_TOKEN.
    pub gh_token: bool,
    /// Ports every new Docker session starts with; the composer can change them per start.
    pub ports: Vec<String>,
}

/// A well-known dev-server port the panel offers as a checkbox.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortPreset {
    /// Container and host port, e.g. 3000.
    pub port: u16,
    /// Frameworks that use it, e.g. „Next.js · Node“.
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GithubSettings {
    /// Folder repositories are cloned into as `<cloneRoot>\<owner>\<name>`;
    /// `''` = [`GithubState::default_clone_root`].
    pub clone_root: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub default_agent: AgentId,
    /// Always one of `environments` and never `wsl` ([`crate::settings::validate`]).
    pub default_env: EnvId,
    /// The environments the composer offers, in [`EnvId::ALL`] order; at least one of them
    /// can start a session (Lokal or Docker). A display preference: recents keep their
    /// environment and restart unchanged.
    pub environments: Vec<EnvId>,
    pub terminal: TerminalId,
    /// Windows Terminal only: every start opens a tab in the one window named `traygent`
    /// (`wt -w traygent`) instead of a new window. No effect on the console.
    pub shared_window: bool,
    /// Defaults per agent.
    pub agents: PerAgent<LaunchOptions>,
    pub docker: DockerSettings,
    pub github: GithubSettings,
}

/// Mirrors `Snapshot` in `src/lib/types.ts`; replaced wholesale on every change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// `true` only for the browser preview's simulated data; the native shell sends `false`.
    pub demo: bool,
    pub settings: AppSettings,
    pub catalog: Vec<AgentCatalogEntry>,
    /// Port checkboxes for Docker sessions, in display order.
    pub port_presets: Vec<PortPreset>,
    /// One per (agent, env) for `local` and `docker`.
    pub agents: Vec<AgentStatus>,
    /// `local` always ready, `docker` probed, `wsl` unavailable (soon).
    pub environments: Vec<EnvStatus>,
    /// Newest first, at most 20.
    pub recents: Vec<RecentSession>,
    /// The GitHub account and its repository listing.
    pub github: GithubState,
    /// True while a detection runs; a `traygent:snapshot` event follows.
    pub detecting: bool,
    /// Increases with every native state change and every finished detection; the panel
    /// drops a snapshot older than the one it shows.
    pub revision: u64,
    /// Persistent German warning, e.g. `state.json` is read-only and nothing is saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    /// RFC 3339 time of the last finished detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOutcome {
    pub recent: RecentSession,
    pub snapshot: Snapshot,
}

#[cfg(test)]
mod tests;
