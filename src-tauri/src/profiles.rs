//! Agent profiles (#33): several sign-ins of one CLI, each in its own config directory, with
//! everything except the sign-in kept identical across them.
//!
//! The model is table-driven ([`Recipe`]); Claude Code is the only agent with one so far. A
//! profile's directory is always built from the user profile, the recipe's fixed names and a
//! validated profile id ([`Homes`]) — no other path ever reaches the sync engine ([`sync`]).
use crate::detect::{host::is_search_dir, Host};
use crate::models::{AgentId, DEFAULT_PROFILE};

pub mod files;
pub mod foreign;
pub mod history;
pub mod json;
pub mod links;
pub mod marker;
pub mod merge;
pub mod node;
pub mod settings;
pub mod share;
pub mod status;
pub mod sync;

pub use links::{system_linker, Linker};
pub use sync::{ProfileReport, SyncOutcome, SyncPlan};

/// Extra profiles per agent.
pub const MAX_PROFILES: usize = 8;
const MAX_ID_CHARS: usize = 24;

/// How one agent keeps profiles: where a profile lives and what its profiles share.
#[derive(Debug, PartialEq, Eq)]
pub struct Recipe {
    /// The environment variable that moves the CLI's home to a profile's directory.
    pub env_var: &'static str,
    /// The CLI's own home under the user profile; a profile `<id>` lives in `<home_dir>-<id>`.
    pub home_dir: &'static str,
    /// The CLI's state file: beside the default home, inside every other profile's directory.
    pub state_file: &'static str,
    /// The sign-in, inside the profile directory. Never read, copied or logged.
    pub credentials: &'static str,
    /// Directories every profile shares: a link to the default home's directory.
    pub shared_dirs: &'static [&'static str],
    /// Files copied between a profile and the default home, the newest winning.
    pub synced_files: &'static [&'static str],
    /// The settings file merged key by key, apart from what belongs to the sign-in.
    pub settings: Option<SettingsRule>,
    /// A line-based history merged as a union.
    pub history: Option<&'static str>,
    /// Top-level keys of the state file every profile shares, merged entry by entry.
    pub shared_keys: &'static [&'static str],
    /// Top-level keys of the state file every profile shares as one value each.
    pub shared_values: &'static [&'static str],
    /// Fields inside the entries of one shared key that stay per profile.
    pub own_fields: Option<OwnFields>,
    /// The arguments that sign a profile in.
    pub login_args: &'static [&'static str],
}

/// Runtime values inside the entries of a shared key of the state file (a project's last
/// session, cost, …): they never travel and never make two entries differ.
#[derive(Debug, PartialEq, Eq)]
pub struct OwnFields {
    /// The shared key whose entries hold them.
    pub key: &'static str,
    /// Field names by prefix and by full name.
    pub prefixes: &'static [&'static str],
    pub names: &'static [&'static str],
}

impl OwnFields {
    pub fn holds(&self, field: &str) -> bool {
        self.names.contains(&field) || self.prefixes.iter().any(|prefix| field.starts_with(prefix))
    }
}

/// A JSON settings file every profile shares except for the keys that carry the sign-in or
/// route requests: those stay per profile, the rest is merged three ways ([`merge`]).
#[derive(Debug, PartialEq, Eq)]
pub struct SettingsRule {
    pub file: &'static str,
    /// Top-level keys that never leave their profile.
    pub own_keys: &'static [&'static str],
    /// The top-level object of environment variables.
    pub env_key: &'static str,
    /// Variables of `env_key` that never leave their profile: by prefix and by full name,
    /// upper case, matched without case.
    pub own_env_prefixes: &'static [&'static str],
    pub own_env_names: &'static [&'static str],
    /// Lists of strings below these paths (keys joined with `.`) are merged as sets.
    pub set_paths: &'static [&'static str],
}

/// Claude Code: one config directory per account through `CLAUDE_CONFIG_DIR`.
const CLAUDE: Recipe = Recipe {
    env_var: "CLAUDE_CONFIG_DIR",
    home_dir: ".claude",
    state_file: ".claude.json",
    credentials: ".credentials.json",
    shared_dirs: &[
        "skills",
        "agents",
        "commands",
        "rules",
        "output-styles",
        "workflows",
        "agent-memory",
        "themes",
        "plugins",
        "projects",
        "file-history",
        "plans",
        "tasks",
        "paste-cache",
        "hooks",
        "todos",
        "session-env",
    ],
    synced_files: &["CLAUDE.md", "keybindings.json"],
    settings: Some(SettingsRule {
        file: "settings.json",
        own_keys: &[
            "apiKeyHelper",
            "forceLoginOrgUUID",
            "forceLoginMethod",
            "awsAuthRefresh",
            "awsCredentialExport",
            "anthropicProfile",
            "anthropicProfiles",
        ],
        env_key: "env",
        own_env_prefixes: &[
            "ANTHROPIC_",
            "CLAUDE_CODE_USE_",
            "AWS_",
            "GOOGLE_",
            "AZURE_",
        ],
        own_env_names: &["CLAUDE_CONFIG_DIR", "CLAUDE_CODE_OAUTH_TOKEN"],
        set_paths: &["permissions.", "autoMode."],
    }),
    history: Some("history.jsonl"),
    shared_keys: &["mcpServers", "projects"],
    shared_values: &["showSpinnerTree", "claudeInChromeDefaultEnabled"],
    own_fields: Some(OwnFields {
        key: "projects",
        prefixes: &["last"],
        names: &["hasUnseenTeamArtifacts"],
    }),
    login_args: &["auth", "login"],
};

/// The agent's profile recipe; `None` for an agent without profiles.
pub fn recipe(agent: AgentId) -> Option<&'static Recipe> {
    match agent {
        AgentId::Claude => Some(&CLAUDE),
        AgentId::Opencode | AgentId::Codex => None,
    }
}

/// `^[a-z0-9][a-z0-9-]{0,23}$` and not `default`: safe as part of a directory name.
pub fn is_valid_id(id: &str) -> bool {
    let safe = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    id != DEFAULT_PROFILE
        && id.len() <= MAX_ID_CHARS
        && id.chars().next().is_some_and(safe)
        && id.chars().all(|c| safe(c) || c == '-')
}

/// Where profiles live on this host: the user profile, absolute and fully expanded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Homes {
    home: String,
    windows: bool,
}

impl Homes {
    /// `None` without a usable user profile (unset, relative or with an unexpanded `%VAR%`).
    pub fn new(home: &str, windows: bool) -> Option<Self> {
        let home = home.trim_end_matches(['\\', '/']);
        is_search_dir(home, windows).then(|| Self {
            home: home.into(),
            windows,
        })
    }

    pub fn of(host: &Host) -> Option<Self> {
        Self::new(host.user_profile.as_deref()?, host.windows)
    }

    fn join(&self, base: &str, part: &str) -> String {
        let sep = if self.windows { '\\' } else { '/' };
        format!("{base}{sep}{part}")
    }

    /// The config directory of the agent's profile `id`: the CLI's own home for `default`,
    /// `<home>-<id>` otherwise. `None` for an agent without profiles or an invalid id.
    pub fn dir(&self, agent: AgentId, id: &str) -> Option<String> {
        let recipe = recipe(agent)?;
        if id == DEFAULT_PROFILE {
            return Some(self.join(&self.home, recipe.home_dir));
        }
        is_valid_id(id).then(|| self.join(&self.home, &format!("{}-{id}", recipe.home_dir)))
    }

    /// The profile's state file: beside the default home, inside any other profile.
    pub fn state_file(&self, agent: AgentId, id: &str) -> Option<String> {
        let recipe = recipe(agent)?;
        if id == DEFAULT_PROFILE {
            return Some(self.join(&self.home, recipe.state_file));
        }
        Some(self.join(&self.dir(agent, id)?, recipe.state_file))
    }
}

#[cfg(test)]
mod tests;
