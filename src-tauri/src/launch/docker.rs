//! `docker run` recipes per agent (docs/AGENT_RECIPES.md §1–§3, "… in Docker").
//!
//! Claude and Codex use the host CLI's config, including its sign-in. Linux tools and
//! OpenCode's state live in named volumes. The init script travels as `TRAYGENT_INIT` (Base64 in the PS
//! script, LF only) and is sourced by a fixed loader, which keeps `"$@"` intact.
use super::quote::{ps_literal, set_env_base64};
use super::Resolved;
use crate::models::{AgentId, AppSettings};
use crate::recents::hex_prefix;
use crate::settings;
use sha2::{Digest, Sha256};

pub mod claude_hooks;
mod home;
pub use home::AgentHome;

pub const CLAUDE_TOOLS: &str = "claude-code-tools";
pub const OPENCODE_BIN: &str = "traygent-opencode-bin";
pub const OPENCODE_DATA: &str = "traygent-opencode-data";
pub const OPENCODE_CONFIG: &str = "traygent-opencode-config";
pub const CODEX_TOOLS: &str = "traygent-codex-tools";
const CONTAINER_PATH: &str = "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
/// Loader verified to keep `"$@"` intact (`opus[1m]`, `a b` arrive as single args).
pub const LOADER: &str = "printenv TRAYGENT_INIT >/tmp/traygent-init.sh && . /tmp/traygent-init.sh";
/// Git uses the same token as gh for github.com over HTTPS. The empty helper value
/// before this one clears helpers inherited from the mounted Windows gitconfig.
pub const GIT_HELPER: &str = "!gh auth git-credential";

/// The volume that holds the agent's CLI; missing means the first start installs it.
pub fn tools_volume(agent: AgentId) -> &'static str {
    match agent {
        AgentId::Claude => CLAUDE_TOOLS,
        AgentId::Opencode => OPENCODE_BIN,
        AgentId::Codex => CODEX_TOOLS,
    }
}

/// Only the Linux tools volume needs ownership changes. Host files retain their permissions.
const CLAUDE_INIT: &[&str] = &[
    "mkdir -p /home/node/.local/bin /home/node/.local/share",
    "chown -R node:node /home/node/.local 2>/dev/null",
    "if [ ! -x /home/node/.local/bin/claude ]; then",
    "  echo '[traygent] Installiere Claude Code ins Volume claude-code-tools ...'",
    "  runuser -u node -- env PATH=\"$agent_path\" bash -c 'curl -fsSL https://claude.ai/install.sh | bash' || exit 1",
    "  [ -x /home/node/.local/bin/claude ] || { echo 'FEHLER: Claude Code Installation fehlgeschlagen'; exit 1; }",
    "fi",
    "exec runuser -u node -- env PATH=\"$agent_path\" /home/node/.local/bin/claude \"$@\"",
];

/// OpenCode V2's official installer into a volume (UNVERIFIED end to end); loopback joins
/// NO_PROXY because the CLI talks to its own local server (ADR-0005).
const OPENCODE_INIT: &[&str] = &[
    "mkdir -p /home/node/.opencode /home/node/.local/share /home/node/.config/opencode",
    "chown -R node:node /home/node/.opencode /home/node/.local /home/node/.config 2>/dev/null",
    "if [ ! -x /home/node/.opencode/bin/opencode ]; then",
    "  echo '[traygent] Installiere OpenCode V2 ins Volume traygent-opencode-bin ...'",
    "  runuser -u node -- env PATH=\"$agent_path\" bash -c 'curl -fsSL https://opencode.ai/v2/install | bash -s -- --no-modify-path' || exit 1",
    "  [ -x /home/node/.opencode/bin/opencode ] || { echo 'FEHLER: OpenCode Installation fehlgeschlagen'; exit 1; }",
    "fi",
    "export NO_PROXY=\"${NO_PROXY:+$NO_PROXY,}localhost,127.0.0.1,::1\"; export no_proxy=\"$NO_PROXY\"",
    "exec runuser -u node -- env PATH=\"$agent_path\" /home/node/.opencode/bin/opencode --standalone \"$@\"",
];

/// An npm shim can survive a missing native package. Probe the binary as the session user
/// and serialize repairs across containers sharing the tools volume.
///
/// On the Docker Desktop bind, files the Windows CLI created appear as root-owned mode 0777.
/// The TUI enforces 0644 on `installation_id` with `fchmod`, which only the owner may call
/// (EPERM, #55). Only that file goes to node; its value stays shared with the host CLI.
const CODEX_INIT: &[&str] = &[
    "chown node:node /home/node/.codex/tmp || exit 1",
    "codex_id=/home/node/.codex/installation_id",
    "if [ -e \"$codex_id\" ] || [ -L \"$codex_id\" ]; then",
    "  [ -f \"$codex_id\" ] && [ ! -L \"$codex_id\" ] && [ \"$(stat -c %h \"$codex_id\")\" = 1 ] || { echo '[traygent] installation_id muss eine reguläre Datei ohne Verknüpfungen sein.' >&2; exit 1; }",
    "  if [ \"$(stat -c %u \"$codex_id\")\" != \"$(id -u node)\" ]; then",
    "    chown -h node:node \"$codex_id\" || { echo '[traygent] Die Dateirechte für die Codex-Installations-ID konnten nicht vorbereitet werden.' >&2; exit 1; }",
    "  fi",
    "fi",
    "mkdir -p /home/node/.local/bin \"$CODEX_SQLITE_HOME\" || exit 1",
    "chown -R node:node /home/node/.local 2>/dev/null",
    "as_codex() { runuser -u node -- /home/node/.local/bin/codex \"$@\"; }",
    "(",
    "  flock -x 9 || exit 1",
    "  if ! as_codex --version >/dev/null 2>&1; then",
    "    echo '[traygent] Installiere oder repariere Codex CLI im Volume traygent-codex-tools ...'",
    "    runuser -u node -- env NPM_CONFIG_PREFIX=/home/node/.local npm install -g --include=optional @openai/codex@latest || exit 1",
    "    as_codex --version || { echo 'FEHLER: Codex CLI konnte nach der Installation nicht gestartet werden'; exit 1; }",
    "  fi",
    ") 9</home/node/.local || exit 1",
    "codex_login() { as_codex -c \"sqlite_home=$CODEX_SQLITE_HOME\" login \"$@\"; }",
    "if ! codex_login status >/dev/null 2>&1; then",
    "  if [ -n \"$OPENAI_API_KEY\" ]; then printenv OPENAI_API_KEY | codex_login --with-api-key",
    "  else codex_login --device-auth; fi || exit 1",
    "fi",
    "codex_cli=(/home/node/.local/bin/codex)",
    "if [[ $(readlink -f /home/node/.local/bin/codex) == *.js ]]; then codex_cli=(\"$(command -v node)\" /home/node/.local/bin/codex); fi",
    "exec runuser -u node -- env PATH=\"$agent_path\" \"${codex_cli[@]}\" -c \"sqlite_home=$CODEX_SQLITE_HOME\" \"$@\"",
];

/// `DOCKER_CLI_FALLBACK_VERSION` of the starter: used when the listing gives no version.
pub const DOCKER_CLI_FALLBACK: &str = "27.5.1";
const DOCKER_STATIC: &str = "https://download.docker.com/linux/static/stable/x86_64/";
/// The host socket's path inside the container, and on the engine's side of the mount.
pub const DOCKER_SOCK: &str = "/var/run/docker.sock";

/// Prepended (as root) when the host Docker socket is passed through (#46): the starter's
/// `INSTALL_DOCKER_LATEST_SH` (static CLI into the agent's `/home/node/.local` volume, so it
/// persists; a failure only warns, and curl's limits end a dropped or stalled download) and
/// `DOCKER_SOCK_INIT` (node joins the socket's group, a non-zero GID only; `chmod a+rw` when
/// node still cannot use it).
fn socket_init() -> Vec<String> {
    let bin = "/home/node/.local/bin";
    let sock = DOCKER_SOCK;
    vec![
        format!("mkdir -p {bin}"),
        format!("if [ ! -x {bin}/docker ]; then"),
        "  echo '[traygent] Installiere Docker CLI ...'".into(),
        format!("  DVER=$(curl -4 -fsSL --connect-timeout 10 --max-time 30 {DOCKER_STATIC} 2>/dev/null | grep -oE 'docker-[0-9]+\\.[0-9]+\\.[0-9]+\\.tgz' | sed 's/docker-//;s/\\.tgz//' | sort -V | tail -1)"),
        format!("  [ -n \"$DVER\" ] || DVER={DOCKER_CLI_FALLBACK}"),
        format!("  if curl -4 -fsSL --connect-timeout 10 --speed-limit 1024 --speed-time 30 \"{DOCKER_STATIC}docker-$DVER.tgz\" | tar xz --strip-components=1 -C {bin} docker/docker && [ -x {bin}/docker ]; then"),
        format!("    chown node:node {bin}/docker; echo \"[traygent] Docker CLI $DVER installiert\""),
        format!("  else rm -f {bin}/docker; echo '[traygent] WARNUNG: Docker CLI konnte nicht installiert werden - docker fehlt im Container'; fi"),
        "fi".into(),
        format!("if [ -S {sock} ]; then"),
        format!("  DSG=$(ls -n {sock} | awk '{{print $4}}')"),
        "  case \"x$DSG\" in x|x0) ;; *)".into(),
        "    getent group \"$DSG\" >/dev/null 2>&1 || groupadd -g \"$DSG\" hostdocker >/dev/null 2>&1".into(),
        "    DGN=$(getent group \"$DSG\" | awk -F: '{print $1}')".into(),
        "    [ -z \"$DGN\" ] || usermod -aG \"$DGN\" node >/dev/null 2>&1;;".into(),
        "  esac".into(),
        format!("  runuser -u node -- sh -c 'test -r {sock} && test -w {sock}' || chmod a+rw {sock} 2>/dev/null"),
        "else echo '[traygent] Kein Docker-Socket im Container - laeuft Docker auf dem Host?'; fi".into(),
    ]
}

/// The init script for the agent, LF line endings whatever the checkout uses; with
/// `socket` the Docker socket block ([`socket_init`]) runs first.
pub fn init_script(agent: AgentId, socket: bool) -> String {
    let lines = match agent {
        AgentId::Claude => CLAUDE_INIT,
        AgentId::Opencode => OPENCODE_INIT,
        AgentId::Codex => CODEX_INIT,
    };
    let mut all = if socket { socket_init() } else { Vec::new() };
    all.push(". /opt/traygent/install-gh.sh || exit 1".into());
    // Only the session user loads or executes tools from its writable cache.
    all.push("agent_path=/home/node/.local/bin:$PATH".into());
    all.push("if [ \"${TRAYGENT_MANAGE_NODE:-0}\" = 1 ]; then".into());
    all.push("  chown -h node:node /home/node/.local || exit 1".into());
    all.push("  node_bin=$(runuser -u node -- /opt/traygent/prepare-node) || { echo '[traygent] Linux-Node konnte nicht vorbereitet werden. Bitte die Meldung oben prüfen.' >&2; exit 1; }".into());
    all.push("  agent_path=${node_bin:+$node_bin:}$agent_path".into());
    all.push("fi".into());
    all.extend(lines.iter().map(|line| line.to_string()));
    let mut script = all.join("\n");
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
/// call (the vault token read), the `docker` arguments and the cleanup that runs
/// in a `finally` however the container ends (drop the fetched token from the session).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DockerRun {
    pub env: Vec<String>,
    pub fetch: Vec<String>,
    pub args: Vec<String>,
    pub cleanup: Vec<String>,
}

/// What a Docker session opens to the host besides its folder: published ports (canonical
/// specs, [`crate::ports::normalize_ports`]) and the host Docker socket (#46).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostAccess<'a> {
    pub ports: &'a [String],
    pub socket: bool,
}

fn push(args: &mut Vec<String>, parts: &[&str]) {
    args.extend(parts.iter().map(|part| part.to_string()));
}

/// The whole `docker run …` argv for `agent` in `folder` (already normalized, no UNC). The
/// image is checked again here: it lands in the argv right before the command, where a value
/// like `--privileged` would turn into a flag. Each of `access.ports` becomes `-p <spec>`
/// right before the image; `access.socket` mounts the host socket and prepends its init.
pub fn run(
    agent: AgentId,
    folder: &str,
    agent_args: Vec<String>,
    opencode_config: Option<&str>,
    access: HostAccess,
    settings: &AppSettings,
    resolved: &Resolved,
) -> Result<DockerRun, String> {
    let image = settings.docker.images.get(agent);
    settings::validate_image(agent, image)?;
    let pdn = project_dir_name(folder);
    // Trust decisions, project MCP servers and session lists can all be keyed by cwd.
    let workspace = format!("/workspace/{pdn}");
    let managed_claude = agent == AgentId::Claude && !settings.profiles.claude.is_empty();
    let mut init = init_script(agent, access.socket);
    if managed_claude {
        // A custom image can supply credentials even when the host forwards none.
        init.insert_str(
            0,
            "unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN CLAUDE_CODE_OAUTH_TOKEN\n",
        );
    }
    let mut env = vec![set_env_base64("TRAYGENT_INIT", &init)];
    let mut fetch = Vec::new();
    let mut cleanup = Vec::new();
    let exe = resolved
        .traygent_exe
        .as_deref()
        .filter(|path| super::is_windows_absolute(path))
        .ok_or("Das Traygent-Programm für die Docker-Umgebung wurde nicht gefunden.")?;
    let (install_dir, _) = exe.rsplit_once(['\\', '/']).expect("absolute executable");
    let resources = format!(r"{install_dir}\docker");
    env.push(format!(
        "if (-not (Test-Path -LiteralPath {} -PathType Leaf)) {{ throw 'Die Docker-Dateien der Traygent-Installation fehlen. Bitte Traygent erneut installieren.' }}",
        ps_literal(&format!(r"{resources}\install-gh.sh"))
    ));
    let mut args = Vec::new();
    push(&mut args, &["run", "--rm", "-it", "--init"]);
    // Root prepares and `runuser` drops to node, neither needs setuid. UNVERIFIED for Claude
    // on a real engine (its installer and auto-update run as node).
    push(&mut args, &["--security-opt", "no-new-privileges"]);
    let name = format!("traygent-{}-{pdn}-{}", agent.id(), resolved.nonce);
    push(&mut args, &["--name", &name]);
    push(&mut args, &["-v", &format!("{folder}:{workspace}")]);
    if access.socket {
        // Read-write: the agent drives the host's daemon (the starter's DOCKER_SOCK_MOUNT).
        push(&mut args, &["-v", &format!("{DOCKER_SOCK}:{DOCKER_SOCK}")]);
    }
    push(&mut args, &["-w", &workspace]);
    push(&mut args, &["-v", &format!("{resources}:/opt/traygent:ro")]);
    let manage_node = if settings.docker.manage_node {
        "1"
    } else {
        "0"
    };
    push(
        &mut args,
        &["-e", &format!("TRAYGENT_MANAGE_NODE={manage_node}")],
    );
    match agent {
        AgentId::Claude => {
            let home = resolved
                .docker_home
                .as_ref()
                .ok_or("Claude-Konfigurationsordner fehlt.")?;
            let source = ps_literal(&home.directory);
            let standard = ps_literal(home.shared_from.as_ref().unwrap_or(&home.directory));
            // DeleteOnClose releases even when the terminal is killed during docker run.
            fetch.push("$tgHookMount=@(); $tgHookFile=[IO.Path]::Combine([IO.Path]::GetTempPath(),('traygent-hooks-'+[guid]::NewGuid().ToString('N')+'.json')); \
                $tgHookLease=[IO.FileStream]::new($tgHookFile,[IO.FileMode]::CreateNew,[IO.FileAccess]::ReadWrite,([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete),4096,[IO.FileOptions]::DeleteOnClose)".into());
            fetch.push(format!(
                "$env:TRAYGENT_HOOK_NODES=(& {} '--docker-claude-hooks' {source} {standard} {} {} $tgHookFile '{manage_node}' | Out-String).Trim(); \
                 if ($LASTEXITCODE -eq 2) {{ Write-Warning 'Claude verwendet die vorhandenen Einstellungen ohne Pfadanpassung.' }} \
                 elseif ($LASTEXITCODE -ne 0) {{ throw 'Die Claude-Hooks für Docker konnten nicht vorbereitet werden.' }}; \
                 if ($tgHookLease.Length) {{ $tgHookMount=@('-v',($tgHookFile+':/home/node/.claude/settings.json:ro')) }}",
                ps_literal(exe), ps_literal(folder), ps_literal(&workspace)
            ));
            cleanup.push("if ($tgHookLease) { $tgHookLease.Dispose() }".into());
            push(&mut args, &["-e", "TRAYGENT_HOOK_NODES"]);
            push(
                &mut args,
                &["-v", &format!("{}:/home/node/.claude:rw", home.directory)],
            );
            if let Some(state) = &home.claude_state {
                push(
                    &mut args,
                    &["-v", &format!("{state}:/home/node/.claude/.claude.json:rw")],
                );
            }
            push(
                &mut args,
                &["-v", &format!("{CLAUDE_TOOLS}:/home/node/.local")],
            );
            push(&mut args, &["-e", "HOME=/home/node"]);
            push(&mut args, &["-e", "CLAUDE_CONFIG_DIR=/home/node/.claude"]);
            if !managed_claude {
                push(
                    &mut args,
                    &["-e", "ANTHROPIC_API_KEY", "-e", "CLAUDE_CODE_OAUTH_TOKEN"],
                );
            }
            push(&mut args, &["-e", CONTAINER_PATH]);
            // Keep the per-folder transcript directory stable across launches.
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
            let home = resolved
                .docker_home
                .as_ref()
                .ok_or("Codex-Konfigurationsordner fehlt.")?;
            push(
                &mut args,
                &["-v", &format!("{CODEX_TOOLS}:/home/node/.local")],
            );
            push(
                &mut args,
                &["-v", &format!("{}:/home/node/.codex:rw", home.directory)],
            );
            push(&mut args, &["-e", "CODEX_HOME=/home/node/.codex"]);
            // Arg0 helpers are executable and require private Unix permissions.
            // A Windows bind cannot supply those or safely share helper cleanup.
            push(
                &mut args,
                &[
                    "--tmpfs",
                    "/home/node/.codex/tmp:rw,exec,nosuid,nodev,mode=0700",
                ],
            );
            // SQLite WAL locks cannot coordinate across the Windows and Linux kernels.
            push(
                &mut args,
                &[
                    "-e",
                    "CODEX_SQLITE_HOME=/home/node/.local/share/traygent-codex/sqlite",
                ],
            );
            push(&mut args, &["-e", "HOME=/home/node", "-e", CONTAINER_PATH]);
            // Name-only: forwarded only when the user's own environment sets it.
            push(&mut args, &["-e", "OPENAI_API_KEY"]);
        }
    }
    // Without these `git commit` has no identity and git refuses the bind mount ("dubious
    // ownership", uid mismatch).
    let count = if settings.docker.gh_token { "4" } else { "2" };
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
    if settings.docker.gh_token {
        use crate::github::docker::{HELPER_ARG, NO_ACCOUNT, UNAVAILABLE};
        let id = resolved
            .github_account_id
            .filter(|id| *id != 0)
            .ok_or(NO_ACCOUNT)?;
        // The explicit pipeline makes PowerShell 5.1 redirect stdout and wait for a
        // GUI-subsystem executable. A plain assignment would detach it without a pipe.
        fetch.push(format!(
            "$env:GH_TOKEN=(& {} {} '{}' 2>$null | Out-String).Trim()",
            ps_literal(exe),
            ps_literal(HELPER_ARG),
            id
        ));
        fetch.push(format!("if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($env:GH_TOKEN)) {{ throw {} }}", ps_literal(UNAVAILABLE)));
        push(
            &mut args,
            &[
                "-e",
                "GIT_CONFIG_KEY_2=credential.https://github.com.helper",
                "-e",
                "GIT_CONFIG_VALUE_2=",
                "-e",
                "GIT_CONFIG_KEY_3=credential.https://github.com.helper",
                "-e",
                &format!("GIT_CONFIG_VALUE_3={GIT_HELPER}"),
                "-e",
                "GH_TOKEN",
            ],
        );
        cleanup.insert(
            0,
            "[Environment]::SetEnvironmentVariable('GH_TOKEN', $null)".to_string(),
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
    for port in access.ports {
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
