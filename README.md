<div align="center">

<img src="src-tauri/icons/source.svg" alt="Traygent logo" width="96" height="96" />

# Traygent

**Coding agents within reach.**<br />
Start Claude Code, OpenCode or Codex from your Windows tray — in one click, locally or in Docker.

[![CI](https://github.com/fo0/traygent/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/fo0/traygent/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Platform: Windows x64](https://img.shields.io/badge/platform-Windows%20x64-0078d4.svg)
![Built with Tauri 2](https://img.shields.io/badge/built%20with-Tauri%202-24c8db.svg)

[Download](https://github.com/fo0/traygent/releases) · [Try the demo](#try-it-in-the-browser) · [User guide](docs/USAGE.md) · [Docs](#documentation)

</div>

| Dark                                                                                                                                              | Light                                                                                                                                               |
| ------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| <img src="docs/images/traygent-dark.png" alt="Traygent panel in the dark theme above its tray icon, with the composer and six recent sessions" /> | <img src="docs/images/traygent-light.png" alt="Traygent panel in the light theme above its tray icon, with the composer and six recent sessions" /> |

<sub>Browser demo with synthetic data on an illustrative taskbar. The UI is in German.</sub>

## Why Traygent

Starting a coding agent usually means: open a terminal, `cd` into the project, remember the right flags, maybe spin up a container. Traygent turns that into **pick → Enter**:

1. Click the tray icon.
2. Pick an agent and a folder — or paste a GitHub repository.
3. Press **Enter**. The session opens in Windows Terminal or a console window.

Every start lands in **Zuletzt** (recent), so the next one is a single click.

## Features

|     |                                    |                                                                                                                               |
| --- | ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| ⚡  | **One-click starts**               | Agent, folder, environment, Enter. Model, effort, permission mode and session mode are chips that default to your settings.   |
| 🕘  | **Recent sessions**                | The last 20 starts as compact rows; a click restarts one with exactly the same settings.                                      |
| 🐳  | **Local or Docker**                | Run the agent on the host or in a container with named volumes for its state and optional port publishing.                    |
| 🐙  | **Start from a GitHub repository** | Paste a URL, type `owner/name` or search your repositories; Traygent clones it on first use and reuses the folder afterwards. |
| 🔍  | **Detects what is installed**      | Agent CLIs, their versions and Docker are detected; a missing CLI offers its install command instead of a dead start.         |
| 🔄  | **Updates in the app**             | Check, read the changelog and install without leaving the app.                                                                |
| 🌗  | **Small and native**               | Rust + Tauri, light and dark theme, starts with Windows if you want it to.                                                    |

Every control is described in the [user guide](docs/USAGE.md).

## Supported agents

| Agent       | Install (opens in a terminal from **Einstellungen → Agenten**) | Permission modes    | Sessions              |
| ----------- | -------------------------------------------------------------- | ------------------- | --------------------- |
| Claude Code | `irm https://claude.ai/install.ps1 \| iex`                     | Ask · Auto · Bypass | New · Continue · Pick |
| OpenCode V2 | `npm install -g @opencode/cli@latest`                          | Ask · Auto · YOLO   | New · Continue        |
| Codex       | `irm https://chatgpt.com/codex/install.ps1 \| iex`             | Ask · Auto · YOLO   | New · Continue · Pick |

How each mode maps to the CLI's own flags: [user guide → Supported agents in detail](docs/USAGE.md#supported-agents-in-detail) and [agent recipes](docs/AGENT_RECIPES.md).

## Get started

### Install

1. Download `Traygent_<version>_x64-setup.exe` from the [Releases page](https://github.com/fo0/traygent/releases).
2. Run it. The installer is **unsigned**, so Windows SmartScreen may ask once: **More info → Run anyway**.
3. Traygent appears in the tray. Later updates install from inside the app.

**Requirements:** Windows x64, Windows PowerShell 5.1 (built in), at least one agent CLI — or let Traygent install it. Optional: Windows Terminal, Docker Desktop, Git (for GitHub repositories).

### Try it in the browser

No Windows needed — the browser demo runs the real panel with simulated data and never starts a process:

```sh
pnpm install --frozen-lockfile
pnpm dev
```

Open http://127.0.0.1:1420. The preview is labelled **Demo**.

## Privacy and security

- **Your GitHub token stays in the Windows Credential Manager.** It only reads your repository list; it never reaches a clone, a command line, an environment variable or the state file. Cloning uses your own Git credentials.
- **No shell strings.** Every launch is an argument list of quoted literals ([ADR-0003](docs/adr/0003-launch-through-a-powershell-argv.md)).
- **Docker ports bind to `127.0.0.1`** unless you type an IP.
- **Network access** is limited to the GitHub API (update check, your repository list) and GitHub release downloads. Agent installers and container images are fetched only in a terminal you open.
- **Local state** lives in one JSON file, `%APPDATA%\io.github.fo0.traygent\state.json` ([details](docs/USAGE.md#where-traygent-keeps-its-state)).

## Limits

- **Windows only.** Elsewhere only the browser demo runs. **WSL** is reserved and shown as „Bald verfügbar“ (coming soon).
- **German UI.** Labels are German; the [user guide](docs/USAGE.md) translates each one.
- **GitHub.com, one account.** GitHub Enterprise hosts are not recognised.
- **PATH after a fresh install.** Terminals Traygent opens inherit its PATH; tools an agent calls itself (for example Git) may need a new login or a Traygent restart.
- **Not yet:** corporate CA import into containers and GPU passthrough.
- Some launch details still await confirmation on a real Windows host; see [BACKLOG.md](BACKLOG.md).

## Development

Built with **Tauri 2, Rust, Svelte 5 and TypeScript**. Prerequisites: **Node.js 24**, **pnpm 11.19**, a stable **Rust** toolchain and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) (on Windows: Microsoft C++ Build Tools, the MSVC Rust toolchain, WebView2).

```sh
pnpm install --frozen-lockfile
pnpm desktop:dev       # run the desktop app
pnpm verify            # lint, typecheck, tests, build
pnpm build:installer   # unsigned NSIS installer (Windows)
```

[CI](.github/workflows/ci.yml) runs lint, typecheck, unit tests, Playwright, Rust fmt/Clippy/tests and a Windows desktop job on every pull request. [Releases](.github/workflows/release.yml) are started by hand on `main` ([release process](docs/RELEASES.md)). README screenshots: `pnpm capture:readme`.

## Documentation

| Topic                                                                                      | For                                          |
| ------------------------------------------------------------------------------------------ | -------------------------------------------- |
| [User guide](docs/USAGE.md)                                                                | every control, GitHub repositories, Docker   |
| [Specification](docs/SPEC.md)                                                              | product, IPC contract, launch rules          |
| [Agent recipes](docs/AGENT_RECIPES.md)                                                     | per-agent flags, install paths, Docker setup |
| [Architecture](docs/ARCHITECTURE.md) · [decisions](docs/adr)                               | how it is built and why                      |
| [Releases and updates](docs/RELEASES.md) · [verification](docs/VALIDATION.md)              | shipping and testing                         |
| [Windows autostart](docs/WINDOWS_AUTOSTART.md) · [popup](docs/WINDOWS_POPUP_VALIDATION.md) | Windows integration details                  |
| [Asset sources](docs/ASSETS.md)                                                            | icon and image provenance                    |

## License

[MIT](LICENSE). Dependencies retain their own licenses. Icons and UI glyphs are drawn from repository-native geometry ([asset sources](docs/ASSETS.md)). The app shell derives from RepoWatch; Traygent replaces the Batch starters of agent-tools for everyday starts.
