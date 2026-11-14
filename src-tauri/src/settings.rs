//! App settings: defaults, normalization and validation (German errors reach the panel).
use crate::models::{
    AgentId, AppSettings, DockerSettings, EnvId, GithubSettings, LaunchOptions, PerAgent,
};
use crate::{catalog, launch, ports};

pub mod profiles;
pub mod share;

/// Base image for every agent's container (docs/AGENT_RECIPES.md §4).
pub const DEFAULT_IMAGE: &str = "node:22-trixie";
const MAX_MODEL_CHARS: usize = 128;
const MAX_EXTRA_ARGS_CHARS: usize = 1000;
const MAX_IMAGE_CHARS: usize = 255;
const WSL_SOON: &str = "WSL ist noch nicht verfügbar.";
const NO_ENVIRONMENT: &str = "Mindestens eine Umgebung muss aktiv sein.";
const DEFAULT_ENV_OFF: &str = "Die Standard-Umgebung ist deaktiviert.";
/// Characters cmd.exe re-parses when a `.cmd` shim forwards its arguments (BatBadBut class);
/// `(` `)` close the `IF EXIST … ( … %* )` block npm shims forward the arguments in.
const SHIM_UNSAFE: [char; 9] = ['&', '|', '<', '>', '^', '%', '!', '(', ')'];
/// ASCII and the typographic quotes PowerShell also treats as string delimiters.
const QUOTES: [char; 10] = [
    '"', '\'', '`', '\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}', '\u{201C}', '\u{201D}',
    '\u{201E}',
];

impl Default for DockerSettings {
    fn default() -> Self {
        Self {
            images: PerAgent::from_fn(|_| DEFAULT_IMAGE.to_string()),
            gh_token: false,
            ports: Vec::new(),
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_agent: AgentId::Claude,
            default_env: EnvId::Local,
            environments: EnvId::ALL.to_vec(),
            terminal: Default::default(),
            shared_window: false,
            agents: PerAgent::from_fn(|_| LaunchOptions::default()),
            docker: DockerSettings::default(),
            github: GithubSettings::default(),
            agent_updates: false,
            profiles: PerAgent::default(),
            agent_profile: PerAgent::from_fn(|_| crate::models::DEFAULT_PROFILE.to_string()),
            profile_share: PerAgent::default(),
            debug: false,
        }
    }
}

/// Trims user-typed text and fills blank values with their defaults.
pub fn normalize(mut settings: AppSettings) -> AppSettings {
    for agent in AgentId::ALL {
        let options = settings.agents.get_mut(agent);
        *options = normalize_options(options.clone());
        let image = settings.docker.images.get_mut(agent);
        *image = image.trim().to_string();
        if image.is_empty() {
            *image = DEFAULT_IMAGE.to_string();
        }
    }
    settings.environments = normalize_environments(&settings.environments);
    // Canonical when valid; an invalid list stays as typed for `validate` to name.
    if let Ok(ports) = ports::normalize_ports(&settings.docker.ports) {
        settings.docker.ports = ports;
    }
    // Canonical when valid, like the ports; `''` means the default clone root.
    let root = settings.github.clone_root.trim();
    settings.github.clone_root = launch::normalize_folder(root).unwrap_or_else(|_| root.into());
    profiles::normalize(&mut settings);
    share::normalize(&mut settings);
    settings
}

/// Each environment once, in [`EnvId::ALL`] order; an empty list stays for `validate`.
pub fn normalize_environments(list: &[EnvId]) -> Vec<EnvId> {
    EnvId::ALL
        .into_iter()
        .filter(|env| list.contains(env))
        .collect()
}

/// Lokal and Docker start sessions; WSL is listed but not available yet.
fn can_start(env: EnvId) -> bool {
    env != EnvId::Wsl
}

/// Trims the free-text fields; a blank model or effort means `default`.
pub fn normalize_options(mut options: LaunchOptions) -> LaunchOptions {
    options.model = options.model.trim().to_string();
    options.effort = options.effort.trim().to_string();
    options.extra_args = options.extra_args.trim().to_string();
    if options.model.is_empty() {
        options.model = "default".into();
    }
    if options.effort.is_empty() {
        options.effort = "default".into();
    }
    options
}

/// Settings read from `state.json`: normalized, then every invalid field takes its default
/// and the rest is kept, so a tampered or outdated file can neither reach a launch nor
/// block later saves.
pub fn sanitize(settings: AppSettings) -> AppSettings {
    let mut settings = normalize(settings);
    let defaults = AppSettings::default();
    if !settings.environments.iter().copied().any(can_start) {
        settings.environments = defaults.environments.clone();
    }
    if !can_start(settings.default_env) || !settings.environments.contains(&settings.default_env) {
        let first = settings
            .environments
            .iter()
            .copied()
            .find(|&env| can_start(env));
        settings.default_env = first.unwrap_or(defaults.default_env);
    }
    for agent in AgentId::ALL {
        let options = settings.agents.get_mut(agent);
        let fallback = LaunchOptions::default();
        if validate_model(agent, &options.model).is_err() {
            options.model = fallback.model;
        }
        if !is_catalog_effort(agent, &options.effort) {
            options.effort = fallback.effort;
        }
        if extra_args(agent, &options.extra_args).is_err() {
            options.extra_args = fallback.extra_args;
        }
        let image = settings.docker.images.get_mut(agent);
        if validate_image(agent, image).is_err() {
            *image = DEFAULT_IMAGE.to_string();
        }
    }
    if ports::normalize_ports(&settings.docker.ports).is_err() {
        settings.docker.ports = defaults.docker.ports;
    }
    if validate_clone_root(&settings.github.clone_root).is_err() {
        settings.github.clone_root = defaults.github.clone_root;
    }
    profiles::sanitize(&mut settings);
    share::sanitize(&mut settings);
    settings
}

pub fn validate(settings: &AppSettings) -> Result<(), String> {
    if settings.default_env == EnvId::Wsl {
        return Err(WSL_SOON.into());
    }
    validate_environments(&settings.environments, settings.default_env)?;
    for agent in AgentId::ALL {
        validate_options(agent, settings.agents.get(agent))?;
        validate_image(agent, settings.docker.images.get(agent))?;
    }
    ports::normalize_ports(&settings.docker.ports).map(drop)?;
    validate_clone_root(&settings.github.clone_root)?;
    profiles::validate(settings)?;
    share::validate(settings)
}

/// At least one environment that can start a session, the default among them.
pub fn validate_environments(list: &[EnvId], default_env: EnvId) -> Result<(), String> {
    if list.is_empty() {
        return Err(NO_ENVIRONMENT.into());
    }
    if !list.iter().copied().any(can_start) {
        return Err(WSL_SOON.into());
    }
    if !list.contains(&default_env) {
        return Err(DEFAULT_ENV_OFF.into());
    }
    Ok(())
}

/// `''` (the default clone root) or an absolute folder [`normalize`] stores normalized; it
/// need not exist yet, a start creates it.
pub fn validate_clone_root(root: &str) -> Result<(), String> {
    if root.is_empty() {
        return Ok(());
    }
    launch::normalize_folder(root)
        .map(drop)
        .map_err(|error| format!("Klon-Ordner: {error}"))
}

/// Checks model, effort and extra arguments against the agent's catalog entry.
pub fn validate_options(agent: AgentId, options: &LaunchOptions) -> Result<(), String> {
    validate_model(agent, &options.model)?;
    if !is_catalog_effort(agent, &options.effort) {
        return Err(format!("Unbekannte Denkstufe für {}.", agent.label()));
    }
    extra_args(agent, &options.extra_args).map(drop)
}

fn is_catalog_effort(agent: AgentId, effort: &str) -> bool {
    catalog::entry(agent)
        .efforts
        .iter()
        .any(|choice| choice.id == effort)
}

fn validate_model(agent: AgentId, model: &str) -> Result<(), String> {
    if model == "default" {
        return Ok(());
    }
    let label = agent.label();
    if model.chars().count() > MAX_MODEL_CHARS {
        return Err(format!("Modell für {label} ist zu lang."));
    }
    // `#` stays out: OpenCode's `#variant` is built from the effort.
    let safe = |c: char| c.is_ascii_alphanumeric() || "._:/@[]-".contains(c);
    // A leading `-` would turn the value into a flag of its own.
    if model.is_empty() || model.starts_with('-') || !model.chars().all(safe) {
        return Err(format!("Modell für {label} enthält unzulässige Zeichen."));
    }
    if agent == AgentId::Opencode && !is_provider_model(model) {
        return Err("Modell für OpenCode als provider/model angeben.".into());
    }
    Ok(())
}

/// OpenCode's `provider/model` form (`^[\w.-]+/[\w.:@/-]+$`); the variant is Traygent's effort.
fn is_provider_model(model: &str) -> bool {
    let Some((provider, name)) = model.split_once('/') else {
        return false;
    };
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    !provider.is_empty()
        && provider.chars().all(|c| word(c) || ".-".contains(c))
        && !name.is_empty()
        && name.chars().all(|c| word(c) || ".:@/-".contains(c))
}

/// Splits the extra CLI arguments on whitespace; every part becomes one quoted literal.
pub fn extra_args(agent: AgentId, raw: &str) -> Result<Vec<String>, String> {
    let label = agent.label();
    if raw.chars().count() > MAX_EXTRA_ARGS_CHARS {
        return Err(format!("Zusatzargumente für {label} sind zu lang."));
    }
    if raw.chars().any(char::is_control) {
        return Err(format!(
            "Zusatzargumente für {label} enthalten Steuerzeichen."
        ));
    }
    if raw.contains(QUOTES) {
        return Err(format!(
            "Zusatzargumente für {label} dürfen keine Anführungszeichen enthalten."
        ));
    }
    Ok(raw.split_whitespace().map(str::to_string).collect())
}

/// A `.cmd` shim hands its arguments to cmd.exe, which would execute these characters.
pub fn check_shim_args(agent: AgentId, args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg.contains(SHIM_UNSAFE)) {
        return Err(format!(
            "Zusatzargumente für {} dürfen bei einer npm-Installation keines der Zeichen & | < > ^ % ! ( ) enthalten.",
            agent.label()
        ));
    }
    Ok(())
}

/// A Docker image reference: ASCII `[A-Za-z0-9._/:@-]`, starting alphanumeric (never a flag).
pub fn is_valid_image(image: &str) -> bool {
    let safe = |c: char| c.is_ascii_alphanumeric() || "._/:@-".contains(c);
    let starts_ok = image
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric());
    image.len() <= MAX_IMAGE_CHARS && starts_ok && image.chars().all(safe)
}

pub fn validate_image(agent: AgentId, image: &str) -> Result<(), String> {
    if !is_valid_image(image) {
        return Err(format!("Docker-Image für {} ist ungültig.", agent.label()));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
