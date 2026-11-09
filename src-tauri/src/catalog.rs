//! The static agent catalog (docs/AGENT_RECIPES.md §1–§4): labels, choices, install commands.
use crate::models::{
    AgentCatalogEntry, AgentId, Choice, EnvId, InstallInfo, PermissionChoice, PermissionId,
    SessionMode,
};

/// Codex models bundled with 0.166.0 (`codex debug models`, visibility `list`, by priority).
const CODEX_MODELS: [&str; 9] = [
    "gpt-6.1-sol",
    "gpt-6-astra",
    "gpt-6-sol",
    "gpt-6-luna",
    "gpt-5.6-sol",
    "gpt-6.1-terra",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
    "gpt-5.5",
];

pub fn catalog() -> Vec<AgentCatalogEntry> {
    AgentId::ALL.into_iter().map(entry).collect()
}

pub fn entry(agent: AgentId) -> AgentCatalogEntry {
    match agent {
        AgentId::Claude => claude(),
        AgentId::Opencode => opencode(),
        AgentId::Codex => codex(),
    }
}

fn choice(id: &str, label: &str, hint: Option<&str>) -> Choice {
    Choice {
        id: id.into(),
        label: label.into(),
        hint: hint.map(Into::into),
    }
}

/// `default` plus the five levels every agent understands.
fn efforts(default_hint: &str) -> Vec<Choice> {
    vec![
        choice("default", "Standard", Some(default_hint)),
        choice("low", "Niedrig", None),
        choice("medium", "Mittel", None),
        choice("high", "Hoch", None),
        choice("xhigh", "Sehr hoch", None),
        choice("max", "Maximal", None),
    ]
}

/// Ask · Auto · bypass; the bypass choice carries the agent's own name for it.
fn permissions(bypass_label: &str, [ask, auto, bypass]: [&str; 3]) -> Vec<PermissionChoice> {
    let permission = |id, label: &str, hint: &str, danger| PermissionChoice {
        id,
        label: label.into(),
        hint: hint.into(),
        danger,
    };
    vec![
        permission(PermissionId::Ask, "Nachfragen", ask, false),
        permission(PermissionId::Auto, "Auto", auto, false),
        permission(PermissionId::Bypass, bypass_label, bypass, true),
    ]
}

fn claude() -> AgentCatalogEntry {
    AgentCatalogEntry {
        id: AgentId::Claude,
        label: AgentId::Claude.label().into(),
        models: vec![
            choice(
                "default",
                "Standard",
                Some("Ohne --model: Standard des Kontos"),
            ),
            choice(
                "fable",
                "Fable 5.2",
                Some("Für die schwersten, längsten Aufgaben"),
            ),
            choice("opus", "Opus 5.5", None),
            choice("sonnet", "Sonnet 5.5", None),
            choice("haiku", "Haiku 5.5", Some("Schnell und günstig")),
            choice(
                "opusplan",
                "Opus Plan",
                Some("Opus beim Planen, sonst Sonnet"),
            ),
        ],
        free_model: true,
        efforts: efforts("Ohne --effort: Standard des Modells"),
        // Claude Code calls this mode `bypassPermissions`.
        permissions: permissions(
            "Bypass",
            [
                "Fragt vor Änderungen und Befehlen",
                "Auto-Modus: sichere Schritte ohne Rückfrage",
                "Überspringt alle Rückfragen",
            ],
        ),
        sessions: vec![SessionMode::New, SessionMode::Continue, SessionMode::Resume],
        environments: vec![EnvId::Local, EnvId::Docker],
        install: InstallInfo {
            command: "irm https://claude.ai/install.ps1 | iex".into(),
            docs_url: "https://code.claude.com/docs/en/setup".into(),
            update: Some("claude update".into()),
            requires: Some("Git for Windows empfohlen".into()),
        },
    }
}

fn opencode() -> AgentCatalogEntry {
    AgentCatalogEntry {
        id: AgentId::Opencode,
        label: AgentId::Opencode.label().into(),
        models: vec![choice(
            "default",
            "Standard",
            Some("Aus der OpenCode-Konfiguration; eigenes Modell als provider/model (opencode models)"),
        )],
        free_model: true,
        efforts: efforts("Nur mit gewähltem Modell (Modell-Variante)"),
        permissions: permissions(
            "YOLO",
            [
                "Regeln aus der OpenCode-Konfiguration",
                "Genehmigt alles, was nicht ausdrücklich verboten ist",
                "Erlaubt jede Aktion ohne Rückfrage",
            ],
        ),
        sessions: vec![SessionMode::New, SessionMode::Continue],
        environments: vec![EnvId::Local, EnvId::Docker],
        install: InstallInfo {
            command: "npm install -g @opencode/cli@latest".into(),
            docs_url: "https://opencode.ai/v2/docs/".into(),
            update: Some("opencode upgrade".into()),
            requires: Some("Node.js + npm".into()),
        },
    }
}

fn codex() -> AgentCatalogEntry {
    let mut models = vec![choice(
        "default",
        "Standard",
        Some("Ohne -m: Standard aus Konto und Konfiguration"),
    )];
    models.extend(CODEX_MODELS.iter().map(|slug| choice(slug, slug, None)));
    AgentCatalogEntry {
        id: AgentId::Codex,
        label: AgentId::Codex.label().into(),
        models,
        free_model: true,
        efforts: efforts("Ohne model_reasoning_effort: Standard des Modells"),
        // Codex's `--yolo` alias.
        permissions: permissions(
            "YOLO",
            [
                "Lokal nur lesen, Änderungen nach Rückfrage; in Docker fragt Codex nur bei Bedarf",
                "Automatische Prüfung, schreibt im Arbeitsordner",
                "Ohne Rückfragen und ohne Sandbox",
            ],
        ),
        sessions: vec![SessionMode::New, SessionMode::Continue, SessionMode::Resume],
        environments: vec![EnvId::Local, EnvId::Docker],
        install: InstallInfo {
            command: "irm https://chatgpt.com/codex/install.ps1 | iex".into(),
            docs_url: "https://github.com/openai/codex".into(),
            update: Some("codex update".into()),
            requires: Some("Windows 10 1809+ mit winget".into()),
        },
    }
}

#[cfg(test)]
mod tests;
