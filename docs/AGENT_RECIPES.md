# Traygent agent recipes (research, 2026-11-03)

Implementation reference for `AgentCatalogEntry`, detection and the argv builder. Sources: agent-tools starters
(read-only, `claude/cli-starter.bat`, the OpenCode starter, ADR-0005/0007/0009), vendor docs (links per
section), and local probes of the real CLIs in a Linux sandbox (`npm view`, `--help`, `--version`,
`opencode debug config` under an egress guard, `codex debug models`). **Verified** = seen in a tool result here.
**UNVERIFIED** = not confirmed (no Windows, no Docker in the research sandbox). Nothing below was run on Windows.

Versions seen: Claude Code npm `2.1.317`; OpenCode `@opencode/cli` `2.2.5` (`latest` tag, bins `opencode` and
`opencode2`); Codex `@openai/codex` `0.166.0`.

## 0. Cross-cutting rules (read first)

1. **Launch by absolute path, never by bare name.** Installers edit the _User_ PATH in the registry; Traygent's own
   process env (and every terminal it spawns) keeps the stale PATH until re-login, so a fresh install shows as
   "missing" and `claude` is "not recognized" in the new window. Detection: search `PATH`, then the registry
   `HKCU\Environment\Path` + `HKLM\...\Session Manager\Environment\Path` (expand `%VARS%`), then the known locations
   below; pass the resolved file to the terminal script (`& 'C:\Users\x\.local\bin\claude.exe' ...`). Only absolute,
   fully expanded PATH entries are searched: a relative entry (`.`, `bin`) would resolve against Traygent's own cwd
   and an unexpanded `%VAR%` is no real directory, so both are skipped; the launch builder refuses any CLI, docker
   or gh path that is not a Windows-absolute path.
2. **Prefer `.exe` over npm `.cmd` shims.** A `.cmd` re-parses its args through cmd.exe (`& | < > ^ %` in an extra arg
   become live — BatBadBut class); PowerShell prefers the `.ps1` shim by name, which `Restricted` ExecutionPolicy
   blocks. For npm installs resolve the real target (OpenCode: `%APPDATA%\npm\node_modules\@opencode\cli\bin\opencode.exe`,
   verified layout on Linux: `bin/opencode.exe` is the binary). Codex's npm bin is `bin/codex.js` (needs node) — use
   the `.cmd` only if no native install exists, and then reject `& | < > ^ % ! ( )` in extra args (`(` `)` close the
   `IF EXIST … ( … %* )` block an npm shim forwards its arguments in).
3. **`powershell.exe` is Windows PowerShell 5.1** — its native-arg passing does not escape embedded `"` (an arg with a
   space and a `"` reaches the exe with the quotes stripped), drops empty-string args, and an arg that contains a space
   and ends in `\` breaks CRT parsing (`"C:\a b\"`). Rules: **no `"` in any native argv element**, no empty args,
   strip trailing `\` from folders before using them in a native arg (a drive root `C:\` only ever
   appears inside `-v C:\:/workspace…`, which does not end in `\`; `Set-Location` takes a PS literal, unaffected).
   JSON or scripts travel through **env vars set from Base64** inside the PS script (alphabet has no `' " ;`):
   `$env:X=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('<b64>'))`. The whole script then reaches
   PowerShell as `-NoLogo -NoExit -EncodedCommand <Base64 of the UTF-16LE script>`: the Base64 alphabet has no
   `;`, `%`, quote or space, so neither wt.exe nor PowerShell's argument parser ever sees one; inside, every value
   stays a single-quoted literal. Over ~30000 encoded characters (Windows caps a command line at 32767) the launch
   fails with a German error. Secrets never appear in the script text: fetch them inside the session and forward
   name-only (`docker run -e GH_TOKEN`; root `MEMORY.md` of agent-tools, verified pattern there). Fetch and call share
   one `try { … } finally { … }`, so the token leaves the session however docker ends:
   `try { $env:GH_TOKEN=(& '<gh.exe>' auth token 2>$null); & '<docker.exe>' …; $tgCode=$LASTEXITCODE }`
   `catch { $tgFailed=$true; throw } finally { [Environment]::SetEnvironmentVariable('GH_TOKEN', $null); <close> }`.
   The same `try` wraps every agent call (local too): `<close>` runs `[Environment]::Exit(0)` unless the call threw
   (`$tgFailed`) or `$tgCode` is set and not 0, 130 or -1073741510 (`0xC000013A`); Ctrl+C stops the script pipeline,
   so `$tgCode` stays `$null` and the `finally` still closes. Any other code falls through to
   `Write-Host ('[traygent] <Agent> wurde mit Code ' + $tgCode + ' beendet.')` and the `-NoExit` prompt
   (UNVERIFIED on Windows PowerShell 5.1, BACKLOG #45).
4. **Env via the spawned process is unreliable with wt.exe**: a `wt` call can hand the tab to an already running
   Windows Terminal process whose env is not Traygent's (UNVERIFIED for current WT). Set env _inside_ the PS script.
5. **Working dir:** none of the three CLIs needs a cwd flag locally —
   `Set-Location -LiteralPath '<folder>' -ErrorAction Stop` in the script (and `wt new-tab -d '<folder>'`, left out
   when the folder contains `;` or `%`, which wt would split at or expand; the tab title gets `,` and `％` instead,
   and `--suppressApplicationTitle` keeps the CLI from replacing it). `-ErrorAction Stop` ends the script
   when the folder is gone, so with `-NoExit` the user lands at a prompt instead of an agent in the wrong folder
   (UNVERIFIED on Windows). Folders are absolute; device (`\\?\`, `\\.\`) and alternate-stream forms (a `:` after
   the drive), `< > " | ? *` and reserved device names (`CON`, `NUL`, `COM1` … with any extension) are refused.
   Codex additionally accepts `-C/--cd <dir>`, OpenCode a positional `[<directory>]` (verified `--help`). Use cwd
   only, for all three.
6. **Version probes:** stdout only, regex `\d+\.\d+\.\d+`, timeout ~5 s, stdin closed. Codex prints a stderr warning
   when `CODEX_HOME` is under a temp dir (seen in sandbox) — ignore stderr.
7. **Docker:** `docker version --format {{.Server.Version}}` (spec). Also check `{{.Server.Os}}` == `linux`: Docker
   Desktop in _Windows containers_ mode answers a version but `node:22-trixie` fails with "no matching manifest".
   docker.exe locations besides PATH: `C:\Program Files\Docker\Docker\resources\bin\docker.exe` (Docker Desktop),
   `%USERPROFILE%\.rd\bin\docker.exe` (Rancher Desktop) (both UNVERIFIED paths, conventional).

## 1. Claude Code (`claude`)

- **Docs:** https://code.claude.com/docs/en/setup · CLI: https://code.claude.com/docs/en/cli-reference
- **Install (official, native, no admin):** `irm https://claude.ai/install.ps1 | iex` (verified docs). Alternatives:
  `winget install Anthropic.ClaudeCode` (no auto-update), `npm install -g @anthropic-ai/claude-code` (Node ≥ 22).
- **Update:** `claude update` (native installs also auto-update in the background; WinGet: `winget upgrade Anthropic.ClaudeCode`).
- **Prerequisites:** Windows 10 1809+. Git for Windows _optional but recommended_: without it Claude uses the
  PowerShell tool instead of Bash (docs). Traygent hint: show „Git for Windows empfohlen“ when `git.exe` is missing.
- **Detect:** `%USERPROFILE%\.local\bin\claude.exe` (native, docs uninstall path), `%APPDATA%\npm\claude.cmd` (npm),
  `%LOCALAPPDATA%\Microsoft\WinGet\Links\claude.exe` (WinGet, UNVERIFIED). `claude --version` → `2.1.317 (Claude Code)`.
- **Model** `--model <alias|id>`; `freeModel: true` (full ids allowed). Catalog (docs model-config, verified):
  `default` (no flag; Opus 5.5 on Pro/Max/Team/API) · `fable` (Fable 5.1, hardest/longest tasks) · `opus` (Opus 5.5) ·
  `sonnet` (Sonnet 5.5) · `haiku` (Haiku 5.5) · `opusplan` (Opus in plan mode, Sonnet otherwise). The starter's
  `[1m]` aliases are no-ops on current models (1M is native from Opus 4.7 / Sonnet 5) — do not offer them.
- **Effort** `--effort <low|medium|high|xhigh|max>` (docs; also `ultracode` ≥ 2.1.203 — omit in v1). `default` = no
  flag (Opus/Sonnet/Haiku 5.5 default `medium`, Fable `high`). Unsupported level → CLI uses the next lower one.
  Starter: `--effort` as the _start_ value, never `CLAUDE_CODE_EFFORT_LEVEL` (that env beats `/effort` all session).
- **Permissions** (starter `PERM_FLAG` + `ALLOW_BYPASS`, lines 822-826 of the CRLF-stripped file):
  | unified                                                                                                           | argv                                                             |
  | ----------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
  | ask                                                                                                               | `--permission-mode default --allow-dangerously-skip-permissions` |
  | auto                                                                                                              | `--permission-mode auto --allow-dangerously-skip-permissions`    |
  | bypass                                                                                                            | `--dangerously-skip-permissions`                                 |
  | The starter knows only auto/bypass (default auto) and appends `--allow-dangerously-skip-permissions` whenever the |
  | flag is not bypass, so Shift+Tab can still reach bypass. `ask` is Traygent's addition (`default` = "Manual"; the  |
  | `manual` alias needs ≥ 2.1.200, so send `default`). If auto mode is unavailable (model, org policy, server-side), |
  | Claude silently starts in Manual (docs). Bypass refuses to run as root on Linux → the container runs as `node`.   |
- **Session:** new = nothing · continue = `--continue` · resume = `--resume` (interactive picker, needs a TTY).
  `--continue` in a folder without sessions errors (starter warns before start).
- **Extra args:** appended last (starter `CLAUDE_EXTRA_ARGS`). Order: perm · model · effort · session · extra.
- **Environments:** `local`, `docker`.

### Claude in Docker (must match the starter — existing login keeps working)

Volumes (ADR-0007/0009, starter `KONFIGURATION`): `claude-code-tools` → `/home/node/.local` (CLI binary),
`claude-code-config` → `/home/node/.claude-volume`, `CLAUDE_CONFIG_DIR=/home/node/.claude-volume/config`, and
`/home/node/.claude` a symlink to that config dir in every container. Never mount the config volume at `~/.claude`
(ADR-0009: settings then land in the unread volume root and the refresh lock leaves the volume → daily `/login`).

```text
docker run --rm -it --init --security-opt no-new-privileges   (UNVERIFIED for Claude on a real engine)
  -v <HOSTDIR>:/workspace
  -v claude-code-config:/home/node/.claude-volume
  -v claude-code-tools:/home/node/.local
  -e HOME=/home/node
  -e CLAUDE_CONFIG_DIR=/home/node/.claude-volume/config
  -e PATH=/home/node/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
  -e CLAUDE_CODE_PROJECT_DIR_NAME=<pdn>          (see below)
  -e CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1        (keeps Traygent's tab title; set locally too)
  -e GIT_CONFIG_COUNT=2 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=*
  -e GIT_CONFIG_KEY_1=core.autocrlf -e GIT_CONFIG_VALUE_1=input
  [-v %USERPROFILE%\.gitconfig:/home/node/.gitconfig:ro]   (only if the file exists)
  [-e GH_TOKEN]                                   (name-only, only when settings.docker.ghToken)
  -e TRAYGENT_INIT -e TERM=xterm-256color -e COLORTERM=truecolor
  -w /workspace
  node:22-trixie
  bash -c 'printenv TRAYGENT_INIT >/tmp/traygent-init.sh && . /tmp/traygent-init.sh' traygent <claude args...>
```

The starter also sets `CLAUDE_CODE_TRUST_WORKSPACE=1`; Traygent does not: with the per-folder project dir the trust
prompt appears once per folder, and skipping it would let a repository's hooks run unasked.

The container starts as root (image default), prepares, then `runuser -u node` (starter); neither needs setuid, so
`no-new-privileges` applies as for the other two. `TRAYGENT_INIT` (set from
Base64 in the PS script, LF only) — derived from the starter's `INSTALL_PROLOGUE_SH`, `INSTALL_CLAUDE_SH`, the
stale-lock cleanup and the main run; `bash -n` clean; the `printenv`/`.` loader verified to keep `"$@"` intact
(`opus[1m]`, `a b` arrive as single args):

```bash
V=/home/node/.claude-volume
mkdir -p /home/node/.local/bin /home/node/.local/share "$V/config"
chown -R node:node /home/node/.local "$V" 2>/dev/null
[ -L /home/node/.claude ] || [ -e /home/node/.claude ] || ln -s "$V/config" /home/node/.claude
find "$V" -maxdepth 2 -name '*.lock' -type d -mmin +60 -exec rm -rf {} + 2>/dev/null
if [ ! -x /home/node/.local/bin/claude ]; then
  echo '[traygent] Installiere Claude Code ins Volume claude-code-tools ...'
  runuser -u node -- bash -c 'curl -fsSL https://claude.ai/install.sh | bash' || exit 1
  [ -x /home/node/.local/bin/claude ] || { echo 'FEHLER: Claude Code Installation fehlgeschlagen'; exit 1; }
fi
cd /workspace || exit 1
exec runuser -u node -- /home/node/.local/bin/claude "$@"
```

**`CLAUDE_CODE_PROJECT_DIR_NAME`** (starter `PROJECT_DIR_INIT`; needs CLI ≥ 2.1.234 and `CLAUDE_CONFIG_DIR` set —
docs confirm the latter): every folder is `/workspace`, so without it all folders share one session list, `--continue`
target and auto-memory. Compute in Rust and pass as `-e` (identical result → Traygent and the Batch starter share the
same per-folder sessions). Algorithm = the bash fragment byte for byte:
`p` = host path, ASCII-lowercased, runs of whitespace collapsed to one space and leading/trailing whitespace dropped
(the fragment's unquoted `echo -n $X`), one trailing `\` or `/` removed · `h` = first 8 hex of SHA-256(`p` as UTF-8,
no newline) · `leaf` = part after the last `\` or `/` · `n` = every byte of `leaf` not in `[a-z0-9_-]` → `-`
(byte-wise: `Ü` → `--`), cut to 40 bytes · result `n-h`. Test vectors (run through the real fragment here):
`C:\Users\Max\My Project\` → `my-project-f7b09a93` · `C:\Code\RepoWatch` → `repowatch-7a494d81` ·
`D:\Ünïcode Ordner` → `--n--code-ordner-66394083`.

**Starter extras — v1 decision:**

| extra                                                                                                                                                                  | v1                                                                                                                                                                                                  | why                                                                                                                                                                                                    |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `.gitconfig:ro` mount + `safe.directory=*`, `core.autocrlf=input`                                                                                                      | **keep**                                                                                                                                                                                            | without them `git commit` fails (no identity) and git refuses the bind mount (“dubious ownership”, uid mismatch)                                                                                       |
| Corp-CA import (Rancher `rd-*.crt` → `update-ca-certificates`; starter default `USE_CORP_CA=true`)                                                                     | drop, BACKLOG                                                                                                                                                                                       | **this is the one that bites**: behind a TLS-inspecting proxy the first-run `curl … install.sh` and the API calls fail with certificate errors. Show the German error hint; spec already backlogs it   |
| TLS insecure (`NODE_TLS_REJECT_UNAUTHORIZED=0` …)                                                                                                                      | drop                                                                                                                                                                                                | opt-in, unsafe                                                                                                                                                                                         |
| GPU stages / Mesa, docker socket + docker CLI, ports, GitNexus, Caveman, repo mode (`claude-code-repos`), init.sh file, host seed/v1 migration (`:init_config_volume`) | drop                                                                                                                                                                                                | opt-in toggles or one-time migrations; a volume the starter already used is v2. A fresh volume just needs one `/login` (credentials are never seeded)                                                  |
| gh CLI + `git-credential-gh`/GCM bridge                                                                                                                                | drop; with `ghToken` on use git only: `-e GIT_CONFIG_KEY_2=credential.https://github.com.helper -e GIT_CONFIG_VALUE_2=!f() { echo username=x-access-token; echo password=$GH_TOKEN; }; f` (COUNT=3) | GH_TOKEN alone does nothing for `git push` (no gh in `node:22-trixie`); this helper needs no install (UNVERIFIED in-container; set from Base64 inside the encoded script, so its `;` never reaches wt) |

## 2. OpenCode V2 (`opencode`)

- **Docs:** https://opencode.ai/v2/docs/ (+ `/cli`, `/cli/config`). Package `@opencode/cli` **verified**
  (`latest` = 2.2.5; `opencode-ai` stays V1). Binary `opencode` (alias `opencode2`, which only V2 installs).
- **Install (Windows):** `npm install -g @opencode/cli@latest` (docs list npm/bun/pnpm/yarn; "Windows package managers
  are not supported"; standalone zips exist: `https://opencode.ai/update/api/latest/cli/opencode/opencode-windows-x64.zip`).
  Prereq: **Node.js + npm**; Git for Windows recommended (binary knows `OPENCODE_GIT_BASH_PATH`, so its shell tool
  uses Git Bash — UNVERIFIED). Note the npm install pulls all four linux/win variant packages (~960 MB on Linux, seen).
- **Update:** `opencode upgrade` (`--method npm|curl|…`, verified help) or `npm install -g @opencode/cli@latest`.
- **Detect:** `%APPDATA%\npm\node_modules\@opencode\cli\bin\opencode.exe` (launch this), shims
  `%APPDATA%\npm\opencode.cmd`/`opencode2.cmd`; curl/Git-Bash installer: `%USERPROFILE%\.opencode\bin\opencode.exe`
  (from `install` script, `INSTALL_DIR=$HOME/.opencode/bin`). `opencode --version` → `opencode v2.2.5`.
  **V1 collision:** an older `opencode-ai` npm install also owns `opencode.cmd` (npm EEXIST on install) and prints a
  bare `1.x` — require major ≥ 2, prefer the `opencode2` shim, else report how to fix it: „OpenCode 1 gefunden –
  zuerst „npm uninstall -g opencode-ai“ ausführen, dann V2 installieren“ for an npm shim, „OpenCode 1 gefunden – alte
  Installation entfernen, dann V2 installieren“ otherwise.
- **Always pass `--standalone`** (ADR-0005; verified flag). Without it the CLI attaches to a background service
  (`opencode serve --service`) that keeps the config/env of _its_ first start — Traygent's per-launch model/permission
  env would be ignored. Seen here: a `debug config` call left such a service running and the next call failed with
  "Managed service port … already in use".
- **Model:** the TUI has **no `--model` flag** (only `run`/`mini` have `-m provider/model#variant`, verified help).
  Use env `OPENCODE_CONFIG_CONTENT` with JSON `{"model":"<provider>/<model>#<variant>"}` — verified with
  `debug config`: it becomes its own layer after the global `opencode.json`, parsed as
  `{providerID, model, variant}` (override precedence over the user's `model` key: UNVERIFIED, likely last-wins).
  Catalog: `models: [default]`, `freeModel: true` (string `provider/model`, validate `^[\w.-]+/[\w.:@/-]+$`). The
  available ids depend on the user's providers (`opencode models` lists them; output is provider-scoped, e.g.
  `github-copilot/claude-sonnet-5.5`, `opencode/big-pickle`) — show a hint, not a fixed list.
- **Effort:** reasoning = model _variant_; ids are per provider/model. Offer `default, low, medium, high, xhigh, max`
  (starter's variant ids) and append `#<effort>` **only when a model is set** (variant syntax needs the model); with
  model `default` hide/ignore the effort chip. Unknown variant behavior: UNVERIFIED.
- **Permissions:** ask = nothing (OpenCode's own rules) · auto = `--auto` ("Auto-approve permissions that are not
  explicitly denied", verified) · bypass = `--auto` + JSON `"permissions":[{"action":"*","resource":"*","effect":"allow"}]`
  in the same `OPENCODE_CONFIG_CONTENT` (starter's yolo rule; how rule arrays of different layers merge: UNVERIFIED).
- **Session:** new = nothing · continue = `--continue` · resume = **unsupported** (no picker flag; `--session <id>`
  only) → `sessions: ['new','continue']`.
- **Extra args:** after the built flags. Order: `--standalone` · `--auto` · `--continue` · extra.
- **Auth:** `opencode auth login` (interactive) — storage path not documented; it lives under the data/config dirs
  (`opencode debug paths`: data `~/.local/share/opencode`, config `~/.config/opencode`, state `~/.local/state/opencode`).

### OpenCode in Docker

The starter builds `opencode-container:latest` from an embedded Dockerfile (decoded: `FROM node:22-trixie`, optional
corp CA, apt `git git-lfs gh curl ca-certificates openssh-client tini`, git-credential wrappers, `npm i -g
@opencode/cli@latest` + delete `@opencode/cli/node_modules`, entrypoint that copies `/etc/opencode/global`, appends
loopback to `NO_PROXY`, `USER node`, `ENTRYPOINT tini -- opencode-entry.sh`, `CMD opencode --standalone`). It is bound
to a company LiteLLM gateway (generated `opencode.json`, `LLM_*` env, per-project `opencode-data-<leaf>-<hash>`).
**Recommendation: do not reuse it** (no build step, no gateway coupling): plain `node:22-trixie` + first-run install
of the single-platform binary into a named volume via the official V2 installer (`--no-modify-path`, installs to
`$HOME/.opencode/bin`; read from the script, not executed here — UNVERIFIED end to end). An official image
`ghcr.io/anomalyco/opencode:<version>` exists (docs) but its user/home layout is unknown — backlog.

```text
docker run --rm -it --init --security-opt no-new-privileges
  -v <HOSTDIR>:/workspace/<pdn>  -w /workspace/<pdn>
  -v traygent-opencode-bin:/home/node/.opencode
  -v traygent-opencode-data:/home/node/.local
  -v traygent-opencode-config:/home/node/.config/opencode
  -e HOME=/home/node -e TRAYGENT_INIT -e OPENCODE_CONFIG_CONTENT -e TERM=xterm-256color -e COLORTERM=truecolor
  [git env / .gitconfig / GH_TOKEN exactly as Claude]
  node:22-trixie
  bash -c 'printenv TRAYGENT_INIT >/tmp/traygent-init.sh && . /tmp/traygent-init.sh' traygent <opencode args...>
```

```bash
mkdir -p /home/node/.opencode /home/node/.local/share /home/node/.config/opencode
chown -R node:node /home/node/.opencode /home/node/.local /home/node/.config 2>/dev/null
if [ ! -x /home/node/.opencode/bin/opencode ]; then
  echo '[traygent] Installiere OpenCode V2 ins Volume traygent-opencode-bin ...'
  runuser -u node -- bash -c 'curl -fsSL https://opencode.ai/v2/install | bash -s -- --no-modify-path' || exit 1
  [ -x /home/node/.opencode/bin/opencode ] || { echo 'FEHLER: OpenCode Installation fehlgeschlagen'; exit 1; }
fi
export NO_PROXY="${NO_PROXY:+$NO_PROXY,}localhost,127.0.0.1,::1"; export no_proxy="$NO_PROXY"
exec runuser -u node -- /home/node/.opencode/bin/opencode --standalone "$@"
```

Why: loopback in `NO_PROXY` — the CLI talks to its own server over local HTTP; a proxy Docker Desktop injects would
swallow it (ADR-0005). Mount at `/workspace/<pdn>` (same `pdn` as Claude) because session lists/continue are
per project/cwd and every folder at `/workspace` would share them (the starter solves it with per-project data
volumes). One shared data volume keeps a single `auth login` for all folders. `--security-opt no-new-privileges`
is the starter's; harmless with root → `runuser` (no setuid needed). `--continue` in docker: per `<pdn>` path.

## 3. OpenAI Codex CLI (`codex`)

- **Docs:** https://github.com/openai/codex (README) · https://learn.chatgpt.com/docs/auth ·
  https://learn.chatgpt.com/docs/agent-approvals-security.md (developers.openai.com/codex/* now 308-redirects there).
- **Install (Windows, official):** `powershell -ExecutionPolicy ByPass -c "irm https://chatgpt.com/codex/install.ps1 | iex"`
  (README, verified). Inside Traygent's PS window: `irm https://chatgpt.com/codex/install.ps1 | iex` with the window
  started as `powershell.exe -NoLogo -NoExit -ExecutionPolicy Bypass -EncodedCommand …`; the script prints
  „Fertig …“ only when `$?` and `$LASTEXITCODE` report success, else a German failure line. Alternative:
  `npm install -g @openai/codex`.
- **Update:** `codex update` (verified subcommand). **Prereq:** Windows 11 recommended, Win10 ≥ 1809, `winget`
  available (docs); the native Windows sandbox may ask for a one-time (elevated) setup on first use.
- **Detect:** `%LOCALAPPDATA%\Programs\OpenAI\Codex\bin\codex.exe` (install.ps1 `$defaultVisibleBinDir`, override
  `CODEX_INSTALL_DIR`; versions under `%USERPROFILE%\.codex\packages\standalone`), npm `%APPDATA%\npm\codex.cmd`.
  `codex --version` → `codex-cli 0.166.0`. Config/auth home `%USERPROFILE%\.codex` (`CODEX_HOME`).
- **Model** `-m <slug>`; `freeModel: true`. Catalog (bundled with 0.166.0 via `codex debug models`, visibility
  `list`, by priority): `gpt-6.1-sol`, `gpt-6-astra`, `gpt-6-sol`, `gpt-6-luna`, `gpt-5.6-sol`, `gpt-6.1-terra`,
  `gpt-5.6-terra`, `gpt-5.6-luna`, `gpt-5.5`. `default` = no flag (which slug that is depends on account/config:
  UNVERIFIED). Better: run `codex debug models` during detection (JSON `{models:[{slug,display_name,
default_reasoning_level,supported_reasoning_levels[{effort}],visibility,priority}]}`), keep `visibility=="list"`.
- **Effort** `-c model_reasoning_effort=<v>`: `low, medium, high, xhigh, max` (all listed models except gpt-5.5 which
  stops at `xhigh`; some add `ultra`). `default` = no flag. The value parses as a raw string when not valid TOML (help).
- **Permissions** (local Windows; flags verified in `--help`, conflicts verified: `--approve-for-me` cannot be combined
  with `--sandbox` or `--ask-for-approval`):
  | unified                                                                                                           | local argv                                                    | docker argv                                                                                                       |
  | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
  | ask                                                                                                               | `--sandbox read-only --ask-for-approval on-request`           | `--sandbox danger-full-access --ask-for-approval on-request`                                                      |
  | auto                                                                                                              | `--approve-for-me` (auto-review, workspace-write)             | `--sandbox danger-full-access --ask-for-approval on-request -c approvals_reviewer=auto_review` (UNVERIFIED combo) |
  | bypass                                                                                                            | `--dangerously-bypass-approvals-and-sandbox` (alias `--yolo`) | same                                                                                                              |
  | Codex's own "Auto" preset is `workspace-write` + `on-request` (edits without asking); `ask` deliberately uses     |
  | read-only so edits prompt. In Docker the Linux sandbox (bwrap + seccomp) typically fails because Docker's default |
  | seccomp profile blocks user namespaces; the docs say to run with `danger-full-access` inside a container — so in  |
  | Docker `ask` only asks when the model requests it (show that in the chip hint).                                   |
- **Session:** new = nothing · continue = subcommand `codex resume --last <flags>` · resume = `codex resume <flags>`
  (picker). Verified: `resume` accepts `-m`, `-c`, `-s`, `-a`, `--approve-for-me`, `-C`. The picker filters by cwd
  (`--all` disables it) → matters for Docker paths.
- **Extra args:** after the built flags. Order: [`resume` [`--last`]] · perm · `-m` · `-c` · extra.
- **Login:** `codex login` (browser, localhost:1455 callback), `codex login --device-auth` (beta, headless),
  `printenv OPENAI_API_KEY | codex login --with-api-key`. Stored in `~/.codex/auth.json` or OS keyring
  (`cli_auth_credentials_store = file|keyring|auto|ephemeral`); treat `auth.json` like a password.

### Codex in Docker (no starter — proposal consistent with the other two)

Browser login cannot work in the container (callback on the container's localhost, no browser) → device code on
first start, or an API key forwarded name-only. With no keyring in the container, `auto` falls back to the file in
the `traygent-codex-home` volume (UNVERIFIED that `auto` is the default store).

```text
docker run --rm -it --init --security-opt no-new-privileges
  -v <HOSTDIR>:/workspace/<pdn>  -w /workspace/<pdn>
  -v traygent-codex-tools:/home/node/.local
  -v traygent-codex-home:/home/node/.codex
  -e HOME=/home/node -e PATH=/home/node/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
  -e TRAYGENT_INIT -e TERM=xterm-256color -e COLORTERM=truecolor [-e OPENAI_API_KEY]
  [git env / .gitconfig / GH_TOKEN exactly as Claude]
  node:22-trixie
  bash -c 'printenv TRAYGENT_INIT >/tmp/traygent-init.sh && . /tmp/traygent-init.sh' traygent <codex args...>
```

```bash
mkdir -p /home/node/.local/bin /home/node/.codex
chown -R node:node /home/node/.local /home/node/.codex 2>/dev/null
if [ ! -x /home/node/.local/bin/codex ]; then
  echo '[traygent] Installiere Codex CLI ins Volume traygent-codex-tools ...'
  runuser -u node -- env NPM_CONFIG_PREFIX=/home/node/.local npm install -g @openai/codex@latest || exit 1
  [ -x /home/node/.local/bin/codex ] || { echo 'FEHLER: Codex Installation fehlgeschlagen'; exit 1; }
fi
if [ ! -s /home/node/.codex/auth.json ]; then
  if [ -n "$OPENAI_API_KEY" ]; then printenv OPENAI_API_KEY | runuser -u node -- /home/node/.local/bin/codex login --with-api-key
  else runuser -u node -- /home/node/.local/bin/codex login --device-auth; fi || exit 1
fi
exec runuser -u node -- /home/node/.local/bin/codex "$@"
```

Notes: npm (node is in the image) instead of `install.sh`, so binaries stay out of the auth volume. Consider
`--no-daemon` in Docker (Codex otherwise uses a shared app-server daemon; pointless in a throwaway container —
UNVERIFIED whether it matters). Device auth may have to be enabled in the ChatGPT account first (UNVERIFIED).
Codex stores trusted projects per path in `config.toml`; `/workspace/<pdn>` keeps trust and `resume --last` per folder.

## 4. Catalog summary (`AgentCatalogEntry`)

| field           | claude                                        | opencode                                                   | codex                                                             |
| --------------- | --------------------------------------------- | ---------------------------------------------------------- | ----------------------------------------------------------------- |
| models          | default, fable, opus, sonnet, haiku, opusplan | default                                                    | default + `codex debug models` list (fallback: the 9 slugs above) |
| freeModel       | true                                          | true (`provider/model`)                                    | true                                                              |
| efforts         | default, low, medium, high, xhigh, max        | default, low, medium, high, xhigh, max (only with a model) | default, low, medium, high, xhigh, max                            |
| sessions        | new, continue, resume                         | new, continue                                              | new, continue, resume                                             |
| environments    | local, docker                                 | local, docker                                              | local, docker                                                     |
| install.command | `irm https://claude.ai/install.ps1 \| iex`    | `npm install -g @opencode/cli@latest`                      | `irm https://chatgpt.com/codex/install.ps1 \| iex`                |
| install.update  | `claude update`                               | `opencode upgrade`                                         | `codex update`                                                    |
| docker image    | `node:22-trixie`                              | `node:22-trixie`                                           | `node:22-trixie`                                                  |

Docker readiness per agent without starting a container: engine `ready` + `docker image inspect node:22-trixie`
(else „Image wird beim ersten Start geladen“) + `docker volume inspect <tools volume>` (else „CLI wird beim ersten
Start installiert“). First start pulls ~0.4 GB image + CLI download: never suppress that output (starter rule
"suppress noise, not duration").

## 5. Pitfalls checklist

- `-v C:\path:/workspace` works with the Windows docker CLI (drive colon handled); spaces fine as one argv element;
  prefer `-v` over `--mount` (commas in paths break `--mount`'s CSV). UNC (`\\server\share`) and `\\wsl$\` paths are
  not bind-mountable from Docker Desktop — reject with a German error. (UNVERIFIED on this machine; standard behavior.)
- `docker run -v <file>` with a missing source silently creates a _directory_ there (starter EISDIR class) — mount
  `.gitconfig` only when it exists as a file.
- CRLF: never write a Batch/Windows-generated script for bash to execute; the env + `printenv` loader keeps it LF
  (starter strips `\r` with `tr -d '\r'` for its init.sh).
- `-it` needs a real console: fine in wt/conhost; never spawn with redirected stdio. `--init` reaps zombies and
  forwards Ctrl+C to the TUI (OpenCode starter uses tini for the same reason).
- `opus[1m]`-style values: safe as argv to `"$@"`; would glob if spliced unquoted into a bash string (starter quotes them).
- wt.exe splits at `;` and may expand `%VAR%` in its arguments: the script travels as `-EncodedCommand` (no `\;`
  escaping needed), `-d` is dropped for a folder with `;` or `%`, and the title has both replaced. The init script
  and the git credential helper value are set from Base64 inside the encoded script.
- A Docker image from `state.json` is validated on load (invalid → `node:22-trixie`), again in the launch builder and
  before `docker image inspect`: it sits right before `bash` in the argv, where `--privileged` would become a flag.
- Claude native auto-update runs inside the container and writes to `claude-code-tools` — fine; an update while
  another session runs keeps the old version alive (versioned dirs).
- Stale `*.lock` dirs from hard-killed containers block token refresh (ADR-0008, claude-code#62600) — keep the 60-min
  cleanup in the init.
- Parallel first starts on an empty tools volume race the installer (two containers installing at once) — UNVERIFIED
  impact; optionally serialize Docker first-starts per agent in Traygent.
