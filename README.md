# Traygent

[![Release](https://github.com/fo0/traygent/actions/workflows/release.yml/badge.svg?branch=main)](https://github.com/fo0/traygent/actions/workflows/release.yml)

<img src="src-tauri/icons/source.svg" alt="" width="64" height="64" align="right" />

**Coding agents within reach.** Traygent is a compact Windows tray app that starts coding-agent sessions — Claude Code, OpenCode V2, Codex — in one click: pick an agent, a folder or a GitHub repository and where it runs (locally or in Docker), press **Starten** or **Enter**, and the session opens in Windows Terminal or a console window. It replaces the Batch starters of [agent-tools](https://github.com/fo0/agent-tools) for everyday starts.

Built with **Tauri 2, Rust, Svelte 5 and TypeScript**. Windows x64 is the target.

| Dark                                                                                                                                              | Light                                                                                                                                               |
| ------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| <img src="docs/images/traygent-dark.png" alt="Traygent panel in the dark theme above its tray icon, with the composer and six recent sessions" /> | <img src="docs/images/traygent-light.png" alt="Traygent panel in the light theme above its tray icon, with the composer and six recent sessions" /> |

<sub>Browser demo with synthetic data on an illustrative taskbar; regenerate with `pnpm capture:readme`.</sub>

## What it does

- **Neue Session — one composer, no prose.** Choose the agent (**Claude Code · OpenCode · Codex**); each carries a status dot for the selected environment (ready or missing). The field starts empty, its placeholder names both forms. Type a folder, browse with the native folder dialog or pick one of the recent folders — or name a GitHub repository in the same field (**Ordner oder Repo**): paste its URL, type `owner/name`, or type a few letters to get suggestions from your connected account ([below](#start-from-a-github-repository)). Choose **Lokal** or **Docker**; **WSL** is shown disabled („Bald verfügbar“). Environments you hide under **Einstellungen → Umgebung → Umgebungen** leave the composer; with just one, the row disappears. The option chips **Modell**, **Denken** (effort; only where the agent has one), **Freigabe** (a symbol that cycles Nachfragen → Auto → bypass, its label in the tooltip, the bypass in a warning colour; the bypass label follows each CLI — **Bypass** for Claude Code, **YOLO** for Codex and OpenCode) **Session** (Neu / Fortsetzen / Auswahl, where the agent supports them) and, for Docker only, **Ports** (checkboxes for common dev-server ports — 3000 Next.js · Node, 5173 Vite, 4200 Angular, 8000 Django · FastAPI, 8080 Spring, 5000 Flask · ASP.NET, 4321 Astro, 6006 Storybook — plus custom Docker specs such as `8080:80` or `3000-3002/udp`) start from the saved defaults and change the next start only. Ports bind to `127.0.0.1` unless an IP is typed explicitly. **Enter** starts. When the agent's CLI is missing locally, the button reads **Installieren …** and opens **Einstellungen → Agenten** for it — never a dead start.
- **Zuletzt — restart in one click.** One single-line row per folder, agent and environment, newest first, at most 20: agent icon, folder name and parent path, then fixed columns at the right edge, so every icon sits in the same column from row to row (an absent part leaves its column empty): published ports, a repository icon when it was started from GitHub, the environment icon, the options as symbols — the chosen model as a one-letter badge (Opus **O**, Sonnet **S**, Haiku **H**; Codex models by their own word: Sol **S**, Astra **A**, Terra **T**, Luna **L**), the thinking level as signal bars — the permission and a relative time. A click anywhere on the row (or **▶**) starts that session again with exactly those settings — once per press, so a double-click never starts it twice. The **⋯** menu offers **In Composer übernehmen**, **Ordner öffnen** (Explorer) and **Entfernen** with **Rückgängig** for 8 seconds.
- **Status at a glance.** The footer shows the Docker state (**Docker 27.5 · bereit**, or why it is not; only while Docker is shown) and when detection last ran (**Stand hh:mm**). Detection runs at startup and on the refresh button, **F5** or **Ctrl+R**, off the UI thread.
- **Einstellungen, staged.** Nothing applies until **Speichern**; **Abbrechen** drops it.
  - **Agenten** — per agent the local status (version or why not) and the Docker readiness, the recommended install command with a copy button and **Installieren** (opens a terminal running it; refresh detection afterwards), the update command, and the defaults: model, effort, permission and extra CLI arguments (validated).
  - **Umgebung** — the environments the composer shows (**Umgebungen**; recents of a hidden one still restart unchanged), default environment and agent, terminal (Windows Terminal or Konsole) with **Ein Fenster für alle Sessions** (Windows Terminal only, off by default: every session opens as a new tab of one shared window), the Docker image per agent and **GitHub-Token weitergeben** (off by default).
  - **GitHub** — the connected account (**login · Token** or **login · gh CLI**) with **Aktualisieren** for its repository list and **Trennen** (inline confirmation); without one, **Lokale Anmeldung prüfen** + **Übernehmen** imports a GitHub CLI sign-in, or a personal access token connects with **Verbinden**. Account actions apply at once; the **Klon-Ordner** is staged like every setting.
  - **App** — theme (System / Hell / Dunkel) and **Mit Windows starten** ([details](docs/WINDOWS_AUTOSTART.md)).
  - **Updates** — in-app updates: **Nach Updates suchen**, at most 5 changelog bullets, **Installieren** ([release process](docs/RELEASES.md)).
- **A panel beside the tray icon.** Placement follows the clicked icon, monitor work area and DPI. Clicking outside hides the panel while preserving open forms; a start hides it so the new terminal gets the focus. **Traygent beenden** in the tray menu exits.

## Supported agents

Traygent maps one permission choice to each agent's own flags and launches the CLI by absolute path. Flags, install paths and the Docker recipes are researched in [docs/AGENT_RECIPES.md](docs/AGENT_RECIPES.md).

| Agent       | Install command (Windows, opens in a terminal)     | Nachfragen / Auto / Bypass · YOLO                                                                                                                                               | Sessions                   | Docker volumes                                                                |
| ----------- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------- | ----------------------------------------------------------------------------- |
| Claude Code | `irm https://claude.ai/install.ps1 \| iex`         | `--permission-mode default` / `--permission-mode auto` (both with `--allow-dangerously-skip-permissions`, so Shift+Tab still reaches bypass) / `--dangerously-skip-permissions` | Neu · Fortsetzen · Auswahl | `claude-code-tools`, `claude-code-config` (shared with the Batch starter)     |
| OpenCode V2 | `npm install -g @opencode/cli@latest`              | the user's config rules / `--auto` / `--auto` plus an allow-all rule in `OPENCODE_CONFIG_CONTENT`; always `--standalone`                                                        | Neu · Fortsetzen           | `traygent-opencode-bin`, `traygent-opencode-data`, `traygent-opencode-config` |
| Codex       | `irm https://chatgpt.com/codex/install.ps1 \| iex` | `--sandbox read-only --ask-for-approval on-request` / `--approve-for-me` / `--dangerously-bypass-approvals-and-sandbox` (in Docker the sandbox is `danger-full-access`)         | Neu · Fortsetzen · Auswahl | `traygent-codex-tools`, `traygent-codex-home`                                 |

The bypass label follows each CLI: **Bypass** for Claude Code, **YOLO** for OpenCode and Codex.

Model and effort travel as `--model` / `--effort` (Claude), in `OPENCODE_CONFIG_CONTENT` as `provider/model#variant` (OpenCode) and as `-m` / `-c model_reasoning_effort=…` (Codex). Extra arguments follow last, one literal each.

## Start from a GitHub repository

The folder field also takes a repository ([ADR-0005](docs/adr/0005-start-from-a-github-repository.md)):

- **Recognised forms** — `https://github.com/<owner>/<repo>` (also `http://`, `www.`, without a scheme, with `.git`, a trailing path, query or fragment), `git@github.com:<owner>/<repo>.git` and a bare `<owner>/<repo>`. An absolute Windows path is still a folder; anything else is search text.
- **Suggestions** — with a connected account, search text lists up to 8 of the account's repositories (owner, collaborator, organisation member; newest push first) whose `owner/name` contains every typed word; **↑/↓**, **Enter** and **Esc** work as in any list. Without an account the list offers **GitHub verbinden …**. A pasted URL or typed `owner/name` needs no account.
- **Clone or reuse** — a repository starts in `<Klon-Ordner>\<owner>\<name>` (default `%USERPROFILE%\Traygent`). An existing folder is reused as it is, and ▶ on a „Zuletzt“ entry keeps using that entry's folder while it exists, even after the Klon-Ordner changed; otherwise the session's own terminal runs `git clone` first, so progress, Git Credential Manager prompts and errors show where the session runs, and a failed clone stops before the agent starts. Docker sessions clone on the host and bind-mount the folder. Git must be installed.
- **One account** — a personal access token or an imported `gh` sign-in (read only after **Übernehmen**), verified against GitHub's `/user`. The token lives in the Windows Credential Manager, never in `state.json`; it is used only to read the repository list and never reaches the clone, a command line or an environment variable. **Trennen** removes Traygent's copy; `gh` stays signed in.
- **The list** — read at startup, after connecting, on **Aktualisieren** and when the field gets focus with a list older than 15 minutes; at most 1000 repositories, kept in memory only.

## Docker sessions

- Every agent runs in `node:22-trixie` (changeable per agent under **Umgebung**) with `docker run --rm -it --init`; the folder is bind-mounted, all agent state lives in **named volumes** ([ADR-0004](docs/adr/0004-docker-volumes-shared-with-the-batch-starter.md)).
- **Claude Code** uses the same volumes and layout as `claude/cli-starter.bat` (`claude-code-tools`, `claude-code-config`, `/workspace`, the same per-folder project name), so an existing container login carries over in both directions.
- **OpenCode** and **Codex** get Traygent-owned volumes and mount the folder at `/workspace/<project-name>`, so their per-folder session lists stay apart.
- The **first start** pulls the image and installs the CLI into its volume; the agent's Docker status says so beforehand. Codex logs in by device code, or by `OPENAI_API_KEY` when your environment sets it (forwarded by name only).
- Git inside the container gets `safe.directory=*`, `core.autocrlf=input` and your `%USERPROFILE%\.gitconfig` read-only. With **GitHub-Token weitergeben** the session fetches `gh auth token` on the host and forwards it as `GH_TOKEN` by name — it never appears in a command line or the state file.
- Network and WSL paths (`\\server\share`, `\\wsl$\…`) cannot be bind-mounted and are refused.

## Where Traygent keeps its state

Settings, recent sessions and the connected GitHub account's login and id live in one versioned JSON file, `%APPDATA%\io.github.fo0.traygent\state.json`, written atomically ([ADR-0002](docs/adr/0002-versioned-json-state-file.md)). A corrupt file is moved aside as `state.json.corrupt-<timestamp>` and replaced by defaults; a file from a newer Traygent is read but never overwritten. The theme stays in the panel's local storage; the GitHub token stays in the Windows Credential Manager (entry `traygent`, user `github-<id>`).

## Install or build

Published Windows installers are listed on the [Releases page](https://github.com/fo0/traygent/releases). The standard installer is **unsigned**; later updates install from inside the app.

### Native development

Prerequisites: **Node.js 24** (`.node-version`), **pnpm 11.19.0** (`packageManager`), a current stable **Rust** toolchain and the [Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/) (on Windows: Microsoft C++ Build Tools, the MSVC Rust toolchain and the WebView2 runtime).

```sh
pnpm install --frozen-lockfile
pnpm desktop:dev
```

To build the unsigned installer **on Windows**: `pnpm build:installer` (output under `src-tauri/target/release/bundle/nsis/`).

### Browser demo

The browser preview needs Node/pnpm only:

```sh
pnpm install --frozen-lockfile
pnpm dev
```

Open **http://127.0.0.1:1420**. The preview is labelled **Demo** and uses simulated data only; it never starts processes on your machine.

## Limits

- **Windows first.** Launching needs Windows PowerShell 5.1 and either Windows Terminal or a console; elsewhere only the browser demo runs.
- **WSL** is reserved and shown as „Bald verfügbar“.
- **GitHub.com only, one account.** GitHub Enterprise hosts are not recognised; repositories outside the account's list are found only when typed as `owner/name` or pasted as a URL. A repository created since the last list read appears after **Aktualisieren**. A token rotated in `gh` must be imported again.
- **Not yet:** corporate CA import into containers and GPU passthrough — `claude/cli-starter.bat` still covers those. Planned work: [BACKLOG.md](BACKLOG.md).
- **Fresh installs and PATH.** Detection finds a CLI installed after Traygent started, but the terminals Traygent opens inherit its PATH; tools an agent calls itself (for example Git) may need a new login or a Traygent restart.
- Several launch details are not yet confirmed on a real Windows host; they are tracked as verification tasks in [BACKLOG.md](BACKLOG.md).

## CI and releases

[CI](.github/workflows/ci.yml) runs on pull requests: lint, typecheck, unit and release-helper tests, the frontend build, Playwright, Rust formatting/Clippy/tests and a Windows job that builds the desktop shell and runs the Win32 popup and isolated autostart-registry tests. [Release](.github/workflows/release.yml) publishes an unsigned Windows EXE only when started by hand on `main` ([release process](docs/RELEASES.md)).

Local verification: [docs/VALIDATION.md](docs/VALIDATION.md).

## Documentation

- [Specification](docs/SPEC.md) — product, IPC contract and launch rules
- [Agent recipes](docs/AGENT_RECIPES.md) — per-agent flags, install paths and Docker recipes
- [Architecture](docs/ARCHITECTURE.md) and its [diagram](docs/ARCHITECTURE.mmd); decisions in [docs/adr](docs/adr)
- [Releases and updates](docs/RELEASES.md), [verification](docs/VALIDATION.md)
- [Windows autostart](docs/WINDOWS_AUTOSTART.md), [Windows popup validation](docs/WINDOWS_POPUP_VALIDATION.md)
- [Asset sources](docs/ASSETS.md)

## License and provenance

[MIT](LICENSE). Dependencies retain their own licenses. Application icons and UI glyphs are defined from repository-native geometry ([Asset sources](docs/ASSETS.md)). The app shell derives from [RepoWatch](https://github.com/fo0/repo-watch); Traygent replaces the Batch starters of [agent-tools](https://github.com/fo0/agent-tools).
