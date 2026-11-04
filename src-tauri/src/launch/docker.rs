//! `docker run` recipes per agent (docs/AGENT_RECIPES.md §1–§3, "… in Docker").
//!
//! State lives in named volumes, never bind-mounts (agent-tools ADR-0007). Claude reuses the
//! Batch starter's volumes and project dir name, so an existing login and the per-folder
//! sessions keep working. The init script travels as `TRAYGENT_INIT` (Base64 in the PS
//! script, LF only) and is sourced by a fixed loader, which keeps `"$@"` intact.
use super::quote::{ps_literal, set_env_base64};
use super::Resolved;
use crate::models::{AgentId, AppSettings};
use crate::recents::hex_prefix;
use crate::settings;
use sha2::{Digest, Sha256};

pub const CLAUDE_TOOLS: &str = "claude-code-tools";
pub const CLAUDE_CONFIG: &str = "claude-code-config";
pub const OPENCODE_BIN: &str = "traygent-opencode-bin";
pub const OPENCODE_DATA: &str = "traygent-opencode-data";
pub const OPENCODE_CONFIG: &str = "traygent-opencode-config";
pub const CODEX_TOOLS: &str = "traygent-codex-tools";
pub const CODEX_HOME: &str = "traygent-codex-home";
const CONTAINER_PATH: &str =
    "PATH=/home/node/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
/// Loader verified to keep `"$@"` intact (`opus[1m]`, `a b` arrive as single args).
pub const LOADER: &str = "printenv TRAYGENT_INIT >/tmp/traygent-init.sh && . /tmp/traygent-init.sh";
/// `credential.https://github.com.helper` value: GH_TOKEN for git without installing gh
/// (UNVERIFIED in-container). It is set from Base64 and forwarded name-only.
pub const GIT_HELPER: &str = "!f() { echo username=x-access-token; echo password=$GH_TOKEN; }; f";

/// The volume that holds the agent's CLI; missing means the first start installs it.
pub fn tools_volume(agent: AgentId) -> &'static str {
    match agent {
        AgentId::Claude => CLAUDE_TOOLS,
        AgentId::Opencode => OPENCODE_BIN,
        AgentId::Codex => CODEX_TOOLS,
    }
}

/// From the starter's `INSTALL_PROLOGUE_SH`, `INSTALL_CLAUDE_SH`, the 60-minute stale-lock
/// cleanup (ADR-0008) and its main run; root prepares, `runuser -u node` runs the CLI.
const CLAUDE_INIT: &[&str] = &[
    "V=/home/node/.claude-volume",
    "mkdir -p /home/node/.local/bin /home/node/.local/share \"$V/config\"",
    "chown -R node:node /home/node/.local \"$V\" 2>/dev/null",
    "[ -L /home/node/.claude ] || [ -e /home/node/.claude ] || ln -s \"$V/config\" /home/node/.claude",
    "find \"$V\" -maxdepth 2 -name '*.lock' -type d -mmin +60 -exec rm -rf {} + 2>/dev/null",
    "if [ ! -x /home/node/.local/bin/claude ]; then",
    "  echo '[traygent] Installiere Claude Code ins Volume claude-code-tools ...'",
    "  runuser -u node -- bash -c 'curl -fsSL https://claude.ai/install.sh | bash' || exit 1",
    "  [ -x /home/node/.local/bin/claude ] || { echo 'FEHLER: Claude Code Installation fehlgeschlagen'; exit 1; }",
    "fi",
    "cd /workspace || exit 1",
    "exec runuser -u node -- /home/node/.local/bin/claude \"$@\"",
];

/// OpenCode V2's official installer into a volume (UNVERIFIED end to end); loopback joins
/// NO_PROXY because the CLI talks to its own local server (ADR-0005).
const OPENCODE_INIT: &[&str] = &[
    "mkdir -p /home/node/.opencode /home/node/.local/share /home/node/.config/opencode",
    "chown -R node:node /home/node/.opencode /home/node/.local /home/node/.config 2>/dev/null",
    "if [ ! -x /home/node/.opencode/bin/opencode ]; then",
    "  echo '[traygent] Installiere OpenCode V2 ins Volume traygent-opencode-bin ...'",
    "  runuser -u node -- bash -c 'curl -fsSL https://opencode.ai/v2/install | bash -s -- --no-modify-path' || exit 1",
    "  [ -x /home/node/.opencode/bin/opencode ] || { echo 'FEHLER: OpenCode Installation fehlgeschlagen'; exit 1; }",
    "fi",
    "export NO_PROXY=\"${NO_PROXY:+$NO_PROXY,}localhost,127.0.0.1,::1\"; export no_proxy=\"$NO_PROXY\"",
    "exec runuser -u node -- /home/node/.opencode/bin/opencode --standalone \"$@\"",
];

/// npm install keeps the binaries out of the auth volume; the first start logs in by API key
/// or device code (device auth may need enabling in the account first — UNVERIFIED).
const CODEX_INIT: &[&str] = &[
    "mkdir -p /home/node/.local/bin /home/node/.codex",
    "chown -R node:node /home/node/.local /home/node/.codex 2>/dev/null",
    "if [ ! -x /home/node/.local/bin/codex ]; then",
    "  echo '[traygent] Installiere Codex CLI ins Volume traygent-codex-tools ...'",
    "  runuser -u node -- env NPM_CONFIG_PREFIX=/home/node/.local npm install -g @openai/codex@latest || exit 1",
    "  [ -x /home/node/.local/bin/codex ] || { echo 'FEHLER: Codex Installation fehlgeschlagen'; exit 1; }",
    "fi",
    "if [ ! -s /home/node/.codex/auth.json ]; then",
    "  if [ -n \"$OPENAI_API_KEY\" ]; then printenv OPENAI_API_KEY | runuser -u node -- /home/node/.local/bin/codex login --with-api-key",
    "  else runuser -u node -- /home/node/.local/bin/codex login --device-auth; fi || exit 1",
    "fi",
    "exec runuser -u node -- /home/node/.local/bin/codex \"$@\"",
];

/// The init script for the agent, LF line endings whatever the checkout uses.
pub fn init_script(agent: AgentId) -> String {
    let lines = match agent {
        AgentId::Claude => CLAUDE_INIT,
        AgentId::Opencode => OPENCODE_INIT,
        AgentId::Codex => CODEX_INIT,
    };
    let mut script = lines.join("\n");
    script.push('\n');
    script
}

/// `CLAUDE_CODE_PROJECT_DIR_NAME`, byte for byte the starter's `PROJECT_DIR_INIT`: the path
/// ASCII-lowercased, whitespace runs collapsed (its unquoted `echo -n $X`), one trailing
/// separator dropped; `<leaf with every byte outside [a-z0-9_-] as '-', max 40>-<sha256:8>`.
pub fn project_dir_name(folder: &str) -> String {
    let lowered = folder.to_ascii_lowercase();
    let collapsed = lowered
        .split([' ', '\t', '\n'])
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let path = collapsed
        .strip_suffix(['\\', '/'])
        .unwrap_or(collapsed.as_str());
    let hash = hex_prefix(&Sha256::digest(path.as_bytes()), 8);
    let leaf = path.rsplit(['\\', '/']).next().unwrap_or(path);
    let allowed =
        |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte);
    let name: String = leaf
        .bytes()
        .map(|byte| if allowed(byte) { char::from(byte) } else { '-' })
        .take(40)
        .collect();
    format!("{name}-{hash}")
}

/// A Docker launch: PS statements that set env vars from Base64, statements right before the
/// call (the gh token fetch or its notice), the `docker` arguments and the cleanup that runs
/// in a `finally` however the container ends (drop the fetched token from the session).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DockerRun {
    pub env: Vec<String>,
    pub fetch: Vec<String>,
    pub args: Vec<String>,
    pub cleanup: Vec<String>,
}

fn push(args: &mut Vec<String>, parts: &[&str]) {
    args.extend(parts.iter().map(|part| part.to_string()));
}

/// The whole `docker run …` argv for `agent` in `folder` (already normalized, no UNC). The
/// image is checked again here: it lands in the argv right before the command, where a value
/// like `--privileged` would turn into a flag. `ports` are canonical specs
/// ([`crate::ports::normalize_ports`]); each becomes `-p <spec>` right before the image.
pub fn run(
    agent: AgentId,
    folder: &str,
    agent_args: Vec<String>,
    opencode_config: Option<&str>,
    ports: &[String],
    settings: &AppSettings,
    resolved: &Resolved,
) -> Result<DockerRun, String> {
    let image = settings.docker.images.get(agent);
    settings::validate_image(agent, image)?;
    let pdn = project_dir_name(folder);
    let workspace = match agent {
        AgentId::Claude => "/workspace".to_string(),
        // Session lists and `--continue` are per cwd; one path per folder keeps them apart.
        AgentId::Opencode | AgentId::Codex => format!("/workspace/{pdn}"),
    };
    let mut env = vec![set_env_base64("TRAYGENT_INIT", &init_script(agent))];
    let mut fetch = Vec::new();
    let mut cleanup = Vec::new();
    let mut args = Vec::new();
    push(&mut args, &["run", "--rm", "-it", "--init"]);
    // Root prepares and `runuser` drops to node, neither needs setuid. UNVERIFIED for Claude
    // on a real engine (its installer and auto-update run as node).
    push(&mut args, &["--security-opt", "no-new-privileges"]);
    let name = format!("traygent-{}-{pdn}-{}", agent.id(), resolved.nonce);
    push(&mut args, &["--name", &name]);
    push(&mut args, &["-v", &format!("{folder}:{workspace}")]);
    match agent {
        AgentId::Claude => {
            push(
                &mut args,
                &["-v", &format!("{CLAUDE_CONFIG}:/home/node/.claude-volume")],
            );
            push(
                &mut args,
                &["-v", &format!("{CLAUDE_TOOLS}:/home/node/.local")],
            );
            push(&mut args, &["-e", "HOME=/home/node"]);
            push(
                &mut args,
                &["-e", "CLAUDE_CONFIG_DIR=/home/node/.claude-volume/config"],
            );
            push(&mut args, &["-e", CONTAINER_PATH]);
            // Needs CLI >= 2.1.234; without it every folder at /workspace shares one session list.
            // No CLAUDE_CODE_TRUST_WORKSPACE (the starter sets it): the trust prompt comes once
            // per folder, and skipping it would let a repo's hooks run unasked.
            push(
                &mut args,
                &["-e", &format!("CLAUDE_CODE_PROJECT_DIR_NAME={pdn}")],
            );
            // Claude Code would replace the tab title with its own (escape sequences pass
            // through `-it`).
            push(&mut args, &["-e", "CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1"]);
        }
        AgentId::Opencode => {
            push(&mut args, &["-w", &workspace]);
            push(
                &mut args,
                &["-v", &format!("{OPENCODE_BIN}:/home/node/.opencode")],
            );
            push(
                &mut args,
                &["-v", &format!("{OPENCODE_DATA}:/home/node/.local")],
            );
            push(
                &mut args,
                &[
                    "-v",
                    &format!("{OPENCODE_CONFIG}:/home/node/.config/opencode"),
                ],
            );
            push(&mut args, &["-e", "HOME=/home/node"]);
            if let Some(config) = opencode_config {
                env.push(set_env_base64("OPENCODE_CONFIG_CONTENT", config));
                push(&mut args, &["-e", "OPENCODE_CONFIG_CONTENT"]);
            }
        }
        AgentId::Codex => {
            push(&mut args, &["-w", &workspace]);
            push(
                &mut args,
                &["-v", &format!("{CODEX_TOOLS}:/home/node/.local")],
            );
            push(
                &mut args,
                &["-v", &format!("{CODEX_HOME}:/home/node/.codex")],
            );
            push(&mut args, &["-e", "HOME=/home/node", "-e", CONTAINER_PATH]);
            // Name-only: forwarded only when the user's own environment sets it.
            push(&mut args, &["-e", "OPENAI_API_KEY"]);
        }
    }
    // Without these `git commit` has no identity and git refuses the bind mount ("dubious
    // ownership", uid mismatch).
    let token = settings
        .docker
        .gh_token
        .then_some(resolved.gh.as_deref())
        .flatten()
        .filter(|gh| super::is_windows_absolute(gh));
    let count = if token.is_some() { "3" } else { "2" };
    push(&mut args, &["-e", &format!("GIT_CONFIG_COUNT={count}")]);
    push(
        &mut args,
        &[
            "-e",
            "GIT_CONFIG_KEY_0=safe.directory",
            "-e",
            "GIT_CONFIG_VALUE_0=*",
        ],
    );
    push(
        &mut args,
        &[
            "-e",
            "GIT_CONFIG_KEY_1=core.autocrlf",
            "-e",
            "GIT_CONFIG_VALUE_1=input",
        ],
    );
    if let Some(gh) = token {
        // The token is fetched inside the session and forwarded name-only: it never appears
        // in the script text, the command line or the snapshot.
        env.push(set_env_base64("GIT_CONFIG_VALUE_2", GIT_HELPER));
        fetch.push(format!(
            "$env:GH_TOKEN=(& {} auth token 2>$null)",
            ps_literal(gh)
        ));
        let key = "GIT_CONFIG_KEY_2=credential.https://github.com.helper";
        push(
            &mut args,
            &["-e", key, "-e", "GIT_CONFIG_VALUE_2", "-e", "GH_TOKEN"],
        );
        cleanup.push("[Environment]::SetEnvironmentVariable('GH_TOKEN', $null)".to_string());
    } else if settings.docker.gh_token {
        fetch.push(
            "Write-Host '[traygent] gh nicht gefunden - GH_TOKEN wird nicht weitergegeben.'".into(),
        );
    }
    // A missing source would make Docker create a directory there (EISDIR class).
    if let Some(gitconfig) = &resolved.gitconfig {
        push(
            &mut args,
            &["-v", &format!("{gitconfig}:/home/node/.gitconfig:ro")],
        );
    }
    push(
        &mut args,
        &[
            "-e",
            "TRAYGENT_INIT",
            "-e",
            "TERM=xterm-256color",
            "-e",
            "COLORTERM=truecolor",
        ],
    );
    if agent == AgentId::Claude {
        push(&mut args, &["-w", &workspace]);
    }
    for port in ports {
        push(&mut args, &["-p", port]);
    }
    push(&mut args, &[image.as_str()]);
    push(&mut args, &["bash", "-c", LOADER, "traygent"]);
    args.extend(agent_args);
    Ok(DockerRun {
        env,
        fetch,
        args,
        cleanup,
    })
}

#[cfg(test)]
mod tests;
