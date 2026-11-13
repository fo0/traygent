# Memory — Long-Term

Stable project knowledge. **Read at session start.**

## Architecture Decisions

### Built on the RepoWatch shell

- **Date:** 2026-11-03
- **Context:** The owner wants a tray app that starts coding-agent sessions fast, on the exact stack and shell of RepoWatch (`fo0/repo-watch`), replacing the Batch starters of `fo0/agent-tools` (`claude/cli-starter.bat` and the OpenCode starter).
- **Decision:** The shell (tray popup, single instance, autostart, in-app updates, theme, staged settings, IPC contract test, CI and release pipeline) was copied from RepoWatch and renamed; every GitHub-domain part was removed. The agent domain (catalog, detection, launch builder, `state.json`, composer, „Zuletzt“, Settings „Agenten“ / „Umgebung“) follows `docs/SPEC.md`; decisions in ADR-0002 (JSON state), ADR-0003 (PowerShell argv), ADR-0004 (Docker volumes).
- **Reason:** A proven, reviewed shell; both repositories were only read, never modified.

### Unsigned Windows releases, versions from the run timestamp

- **Date:** 2026-11-03 (inherited)
- **Decision:** The manual _Release_ Action publishes an unsigned NSIS EXE (`createUpdaterArtifacts: false` in `src-tauri/tauri.release.conf.json`). The version is `YYYY.MMDD.HHmm` (UTC) from the run creation time, stamped only into the build workspace — Git keeps `0.1.0`. Detail: `docs/RELEASES.md`.

### Unsigned builds update in the app

- **Date:** 2026-11-03 (inherited)
- **Decision:** Unsigned builds download the release EXE `Traygent_<version>_x64-setup.exe`, verify size + SHA-256 against GitHub's asset digest and run the NSIS installer, which restarts the app without `--autostart`. Never send the user to a web page to update (`agent_docs/ux_rules.md` rule 9).

### Start from a GitHub repository

- **Date:** 2026-11-04
- **Decision:** The folder field also takes a GitHub repository (URL, `git@…`, `owner/name`) or search text over the one account's `/user/repos` listing; a start reuses `<clone root>\<owner>\<name>` or clones into it in the session's own script. Detail and alternatives: [ADR-0005](docs/adr/0005-start-from-a-github-repository.md).

### Environments can be hidden

- **Date:** 2026-11-04
- **Decision:** `settings.environments` lists what the composer shows (default all three); display only — recents of a hidden environment still restart, Docker is still probed. Lokal or Docker must stay on, the default must be shown; unticking Docker resets its invalid fields. Detail: [ADR-0007](docs/adr/0007-persisted-environment-visibility.md).

### Agent profiles share everything but the sign-in

- **Date:** 2026-11-13
- **Decision:** One config dir per sign-in (`CLAUDE_CONFIG_DIR=%USERPROFILE%\.claude-<id>`, „Standard“ = `.claude`); shared dirs (+ user extras) are junctions into the default home; `settings.json` and the `.claude.json` subset merge three-way against a value base, sign-in keys per profile; other files whole (newest wins). Claude Code, local only. Detail: [ADR-0009](docs/adr/0009-agent-profiles-with-linked-config-directories.md), [ADR-0010](docs/adr/0010-three-way-merge-of-the-profiles-settings-and-state-file.md), `docs/SPEC.md → Profiles`.

## Gotchas & Pitfalls

- **Releases are manual only** — _Actions → Release → Run workflow_ on `main` runs `ci.yml` for that commit, then builds and publishes; merges never release. Never start one unprompted. (2026-11-03)
- **`pnpm lint` is `prettier --check . && eslint .`** — prettier covers Markdown and JSON too. The husky pre-commit hook (lint-staged → prettier, `rustfmt` for `*.rs`) is installed by `pnpm install`. Never `--no-verify`. (2026-11-03)
- **Cargo features** — `default = []` (serde, serde_json, chrono, and `sha2` as a regular dependency: recent ids, Docker project dir name, update digests), `updates` (reqwest, semver), `vault` (keyring; `KeyringVault` compiles only with it, `--features updates,vault` type-checks it on Linux), `desktop` (Tauri + plugins, pulls `updates` + `vault`). Linux checks use `--features updates`; `--features desktop` is built and tested in the Windows CI job. With `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev` and a built `dist/`, `cargo clippy --features desktop` also runs on Linux. (2026-11-03)
- **Type-check `cfg(windows)` code from Linux** — `rustup target add x86_64-pc-windows-gnu`, `apt-get install gcc-mingw-w64-x86-64`, then `cargo clippy --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-gnu --features desktop --all-targets -- -D warnings` (needs a built `dist/`). It checks, it does not run. (2026-11-03)
- **Golden snapshot fixture** — `fixtures/snapshot.default.json` is written by a Rust test and checked against `Snapshot` by `src/lib/fixture.test.ts`. After a model or catalog change: `TRAYGENT_UPDATE_FIXTURES=1 cargo test --manifest-path src-tauri/Cargo.toml --features updates the_default_native_snapshot`, then `pnpm exec prettier --write fixtures/snapshot.default.json`. (2026-11-03)
- **Spawned terminals inherit Traygent's PATH** — detection also reads the registry PATH, so a fresh install is found and launched by absolute path, but tools the agent calls itself (Git, node) stay missing until a re-login or a Traygent restart. A registry PATH refresh for the terminal is in BACKLOG. (2026-11-03)
- **PowerShell 5.1 quoting** — no `"` and no empty element in any native argv, no trailing `\` on folders, JSON/scripts only as Base64 env, `;` as `\;` for wt.exe, secrets name-only: [ADR-0003](docs/adr/0003-launch-through-a-powershell-argv.md). Never build a cmd string. (2026-11-03)
- **`release:test` skips the workflow tests without PowerShell 7**; `POWERSHELL_EXECUTABLE=<path>/pwsh` runs them for real. Every `run:` line of `release.yml` executes there with mocked `gh`, `pnpm`, `cargo`, `rustup` — a new tool in a release step needs a mock. (2026-11-03)
- **Popup dismissal is generation-guarded**: blur hides after 120 ms, every tray gesture/focus/reopen bumps a generation that cancels stale hides — keep that when touching `tray_popup.rs` / `popup.rs`. (2026-11-03)
- **Release git reads run without credentials** — they rely on the repository being public; if it goes private, pass the token explicitly. (2026-11-03)
- **PR checks skip `rust-core` / `windows-desktop`** when the PR touches none of their paths (`changes` job in `ci.yml`); extend its patterns when Rust or release code starts reading files elsewhere. (2026-11-03)
- **The GitHub token never reaches the clone** — it only reads `/user` and `/user/repos`; `git clone` runs in the session script on the user's own Git credentials (GCM). Never put it in a URL, env var, credential helper or `state.json` (vault entry `traygent` / `github-<id>`). (2026-11-04)
- **gh has its own runner** (`github/process.rs`), not `detect::SystemRunner`: that one truncates output (a cut token), inherits the env (an inherited `GH_TOKEN` wins over the chosen sign-in), times out at 5 s; this one: minimal env, fails over 64 KiB, 20 s. (2026-11-04)
- **`Link` header: split at `<…>`, not at commas** — GitHub echoes `affiliation=owner,collaborator,…` inside the next URL (`github/api.rs → next_page`). (2026-11-04)
- **Cloud container setup** — Node 24 and PowerShell 7 are not preinstalled; Playwright needs `PLAYWRIGHT_CHROMIUM_EXECUTABLE=/opt/pw-browsers/chromium-<rev>/chrome-linux64/chrome`. Steps: `agent_docs/testing.md → Cloud container`. (2026-11-03)
- **Colours live in `src/styles/00-tokens.css` only** (dark set plus two identical light blocks); `tokens.test.ts` rejects literals elsewhere and checks WCAG AA pairs. Playwright runs dark by default (`playwright.config.ts`). (2026-11-03)
- **App icons come from geometry** in `scripts/generate-icons.mjs` (writes `src-tauri/icons/*`, `source.svg` included); `pnpm icons:check` fails on drift. The tray icon embeds `icons/32x32.png`. (2026-11-03)

- **Agent CLI updates fail closed** — the in-use check (`agent_updates.rs → LIST_SCRIPT`) needs its end marker and only known lines; a new line kind in the script must be added to `line_agent`, or every agent reads as in use. (2026-11-10)

- **`store.rs → parse_settings` reads settings field by field** — a new `AppSettings` field needs its line there (plus `sanitize` when it reaches a path) or a saved value is dropped on load. (2026-11-10)

- **Profiles: `profiles.rs → Homes` is the only path builder** — user profile + recipe names + validated id; extras only after `profiles/share.rs → check_name`. Only the recipe's shared part crosses profiles (`settings.json` minus `SettingsRule`, `.claude.json` `shared_keys` + `shared_values` minus `OwnFields`); never `.credentials.json` — keep it so in a new recipe. Bases in `<profile>\.traygent-base\<file>` hold values (`mcpServers` commands and `env` too) and stay in their own profile dir — never link, sync or back them up; the marker's name-only `base` is legacy, read once. Engine tests link via `testing::TestLinker` (symlinks, BACKLOG #60); only one Windows test runs `JunctionLinker`. Sync never holds the service lock (plan → `run_plan` → `apply_profiles` / `finish_launch_sync`). Losers go to the backup of their own home; a foreign `.claude-<id>` is never touched. Detail: `docs/SPEC.md → Profiles`. (2026-11-13)

## External Dependencies

- **GitHub Releases API** (`api.github.com/repos/fo0/traygent/releases/latest`, unauthenticated) for the update check; downloads only from GitHub hosts. (2026-11-03)
- **GitHub REST API, authenticated** — `/user` and `/user/repos` with the user's token; the token lives in **keyring** v4 (`v1` stores, Windows Credential Manager), feature `vault`, failing closed without it. (2026-11-04)
- **Agent installers and images** — install commands (`claude.ai`, `chatgpt.com/codex`, npm `@opencode/cli`) run only in a terminal the user opens; Docker sessions pull `node:22-trixie` and install the CLI into a named volume on first start ([ADR-0004](docs/adr/0004-docker-volumes-shared-with-the-batch-starter.md)). (2026-11-03)
- **RepoWatch** (`fo0/repo-watch`) and **agent-tools** (`fo0/agent-tools`) are read-only references, never modified or bundled. (2026-11-03)

## User Preferences

- **Chat in German, direct, no filler**; generated files and commits in English (owner, 2026-11-03)
- **Compact „Ein Blick“ UI** — the UX rules in `agent_docs/ux_rules.md` are binding (owner, 2026-11-03)
