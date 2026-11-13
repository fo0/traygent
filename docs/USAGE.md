# User guide

Everything the panel does, control by control. The UI labels are German; each one is quoted as it appears, with its meaning where it is not obvious. Back to the [README](../README.md).

- [New session](#new-session--neue-session)
- [Recent sessions](#recent-sessions--zuletzt)
- [Status and refresh](#status-and-refresh)
- [Settings](#settings--einstellungen)
- [Panel and tray](#panel-and-tray)
- [Supported agents in detail](#supported-agents-in-detail)
- [Start from a GitHub repository](#start-from-a-github-repository)
- [Several accounts — „Profile“](#several-accounts--profile)
- [Docker sessions](#docker-sessions)
- [Where Traygent keeps its state](#where-traygent-keeps-its-state)

## New session — „Neue Session“

One composer, no prose:

- **Agent** — **Claude Code · OpenCode · Codex**; each carries a status dot for the selected environment (ready or missing).
- **Folder or repository** („Ordner oder Repo“) — the field starts empty, its placeholder names both forms. Type a folder, browse with the native folder dialog or pick one of the recent folders — or name a GitHub repository: paste its URL, type `owner/name`, or type a few letters to get suggestions from your connected account ([below](#start-from-a-github-repository)). **Ctrl+L** jumps to the field and selects its text.
- **Environment** — **Lokal** (local) or **Docker**; **WSL** is shown disabled („Bald verfügbar“, coming soon). Environments you hide under **Einstellungen → Umgebung → Umgebungen** leave the composer; with just one, the row disappears.
- **Option chips** — start from the saved defaults and change the next start only:
  - **Modell** — the model.
  - **Denken** — the effort level; only where the agent has one.
  - **Freigabe** — the permission mode: a symbol that cycles **Nachfragen** (ask) → **Auto** → bypass, its label in the tooltip, the bypass in a warning colour. The bypass label follows each CLI — **Bypass** for Claude Code, **YOLO** for Codex and OpenCode.
  - **Session** — **Neu** (new) / **Fortsetzen** (continue) / **Auswahl** (pick from the agent's session list), where the agent supports them.
  - **Profil** — Lokal only, once the agent has a second profile ([below](#several-accounts--profile)): a round mark — a person for **Standard**, else the profile's initial — that picks which sign-in the next start uses; in a warning colour while that profile is not signed in.
  - **Ports** — Docker only: checkboxes for common dev-server ports (3000 Next.js · Node, 5173 Vite, 4200 Angular, 8000 Django · FastAPI, 8080 Spring, 5000 Flask · ASP.NET, 4321 Astro, 6006 Storybook) plus custom Docker specs such as `8080:80` or `3000-3002/udp`. Ports bind to `127.0.0.1` unless an IP is typed explicitly.
- **Starten** (start) or **Enter** opens the session. When the agent's CLI is missing locally, the button reads **Installieren …** and opens **Einstellungen → Agenten** for it — never a dead start.

## Recent sessions — „Zuletzt“

- One single-line row per folder, agent and environment, newest first, at most 20.
- Each row shows the agent icon (with the profile's initial on its corner when it started with a profile other than **Standard**), folder name and parent path, then fixed columns at the right edge, so every icon sits in the same column from row to row (an absent part leaves its column empty): published ports, a repository icon when it was started from GitHub, the environment icon, the options as symbols and a relative time.
- Options as symbols: the chosen model as a one-letter badge (Opus **O**, Sonnet **S**, Haiku **H**; Codex models by their own word: Sol **S**, Astra **A**, Terra **T**, Luna **L**), the thinking level as signal bars, the permission as a symbol.
- A click anywhere on the row (or **▶**) starts that session again with exactly those settings — once per press, so a double-click never starts it twice.
- The **⋯** menu offers **In Composer übernehmen** (copy into the composer), **Bearbeiten …** (edit the model, thinking level, permission, extra arguments and Docker ports the row starts with, without starting it), **Ordner öffnen** (open the folder in Explorer), **Pfad kopieren** (copy the folder path) and **Entfernen** (remove) with **Rückgängig** (undo) for 8 seconds.

## Status and refresh

- The footer shows the Docker state (**Docker 27.5 · bereit** = ready, or why it is not; only while Docker is shown) and when detection last ran (**Stand hh:mm** = as of hh:mm).
- Detection runs at startup and on the refresh button, **F5** or **Ctrl+R**, off the UI thread.

## Settings — „Einstellungen“

Nothing applies until **Speichern** (save); **Abbrechen** (cancel) drops it.

- **Agenten** (agents) — per agent the local status (version or why not) and the Docker readiness, the recommended install command with a copy button and **Installieren** (install; opens a terminal running it; refresh detection afterwards), the update command, and the defaults: model, effort, permission and extra CLI arguments (validated). Claude Code also has **Profile** ([below](#several-accounts--profile)).
- **Umgebung** (environment) — the environments the composer shows (**Umgebungen** = environments; recents of a hidden one still restart unchanged), default environment and agent, terminal (Windows Terminal or Konsole = classic console) with **Ein Fenster für alle Sessions** (one window for all sessions; Windows Terminal only, off by default: every session opens as a new tab of one shared window), the Docker image per agent and **GitHub-Token weitergeben** (forward the GitHub token; off by default).
- **GitHub** — the connected account (**login · Token** or **login · gh CLI**) with **Aktualisieren** (refresh) for its repository list and **Trennen** (disconnect, inline confirmation); without one, **Lokale Anmeldung prüfen** (check local sign-in) + **Übernehmen** (adopt) imports a GitHub CLI sign-in, or a personal access token connects with **Verbinden** (connect). Account actions apply at once; the **Klon-Ordner** (clone folder) is staged like every setting.
- **App** — theme (System / Hell = light / Dunkel = dark) and **Mit Windows starten** (start with Windows, [details](WINDOWS_AUTOSTART.md)).
- **Updates** — in-app updates: **Nach Updates suchen** (check for updates), at most 5 changelog bullets, **Installieren** ([release process](RELEASES.md)). Below it **Agenten**: **Automatisch aktualisieren** (off by default) keeps the installed agent CLIs up to date — shortly after the app starts and every 4 hours, each CLI's own update command runs in the background, skipped while that CLI is in use; **Jetzt aktualisieren** runs it at once, and one line per agent shows the last result.

## Panel and tray

The panel opens beside the tray icon; placement follows the clicked icon, monitor work area and DPI. Clicking outside hides the panel while preserving open forms; a start hides it so the new terminal gets the focus. **Traygent beenden** (quit Traygent) in the tray menu exits.

## Supported agents in detail

Traygent maps one permission choice to each agent's own flags and launches the CLI by absolute path. Flags, install paths and the Docker recipes are researched in [AGENT_RECIPES.md](AGENT_RECIPES.md).

| Agent       | Nachfragen / Auto / Bypass · YOLO                                                                                                                                               | Sessions                   | Docker volumes                                                                |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------- | ----------------------------------------------------------------------------- |
| Claude Code | `--permission-mode default` / `--permission-mode auto` (both with `--allow-dangerously-skip-permissions`, so Shift+Tab still reaches bypass) / `--dangerously-skip-permissions` | Neu · Fortsetzen · Auswahl | `claude-code-tools`, `claude-code-config` (shared with the Batch starter)     |
| OpenCode V2 | the user's config rules / `--auto` / `--auto` plus an allow-all rule in `OPENCODE_CONFIG_CONTENT`; always `--standalone`                                                        | Neu · Fortsetzen           | `traygent-opencode-bin`, `traygent-opencode-data`, `traygent-opencode-config` |
| Codex       | `--sandbox read-only --ask-for-approval on-request` / `--approve-for-me` / `--dangerously-bypass-approvals-and-sandbox` (in Docker the sandbox is `danger-full-access`)         | Neu · Fortsetzen · Auswahl | `traygent-codex-tools`, `traygent-codex-home`                                 |

Model and effort travel as `--model` / `--effort` (Claude), in `OPENCODE_CONFIG_CONTENT` as `provider/model#variant` (OpenCode) and as `-m` / `-c model_reasoning_effort=…` (Codex). Extra arguments follow last, one literal each.

## Start from a GitHub repository

The folder field also takes a repository ([ADR-0005](adr/0005-start-from-a-github-repository.md)):

- **Recognised forms** — `https://github.com/<owner>/<repo>` (also `http://`, `www.`, without a scheme, with `.git`, a trailing path, query or fragment), `git@github.com:<owner>/<repo>.git` and a bare `<owner>/<repo>`. An absolute Windows path is still a folder; anything else is search text.
- **Suggestions** — with a connected account, search text lists up to 8 of the account's repositories (owner, collaborator, organisation member; newest push first) whose `owner/name` contains every typed word; **↑/↓**, **Enter** and **Esc** work as in any list. Without an account the list offers **GitHub verbinden …** (connect GitHub). A pasted URL or typed `owner/name` needs no account.
- **Clone or reuse** — a repository starts in `<Klon-Ordner>\<owner>\<name>` (default `%USERPROFILE%\Traygent`). An existing folder is reused as it is, and ▶ on a „Zuletzt“ entry keeps using that entry's folder while it exists, even after the Klon-Ordner changed; otherwise the session's own terminal runs `git clone` first, so progress, Git Credential Manager prompts and errors show where the session runs, and a failed clone stops before the agent starts. Docker sessions clone on the host and bind-mount the folder. Git must be installed.
- **One account** — a personal access token or an imported `gh` sign-in (read only after **Übernehmen**), verified against GitHub's `/user`. The token lives in the Windows Credential Manager, never in `state.json`; it is used only to read the repository list and never reaches the clone, a command line or an environment variable. **Trennen** removes Traygent's copy; `gh` stays signed in.
- **Limits** — repositories outside the account's list are found only when typed as `owner/name` or pasted as a URL; a repository created since the last list read appears after **Aktualisieren**; a token rotated in `gh` must be imported again.
- **The list** — read at startup, after connecting, on **Aktualisieren** and when the field gets focus with a list older than 15 minutes; at most 1000 repositories, kept in memory only.

## Several accounts — „Profile“

Claude Code can keep several sign-ins side by side, say a private and a team subscription ([ADR-0009](adr/0009-agent-profiles-with-linked-config-directories.md)). Each profile has its own login; skills, agents, commands, plugins, settings, MCP servers and history stay the same in all of them ([ADR-0010](adr/0010-three-way-merge-of-the-profiles-settings-and-state-file.md)).

1. **Add** — **Einstellungen → Agenten → Claude Code → Profile**: type a name into **Neues Profil**, press **Enter** or **+**, then **Speichern**. The name gives the folder `%USERPROFILE%\.claude-<name>` (the field's tooltip shows it); a profile named **Team** adopts an existing `.claude-team` together with its login. A folder of that name that is not a Claude Code folder (no `.claude.json`, no login) is left alone: the row says „Profilordner gehört nicht zu Claude Code“ and the profile neither starts nor signs in — pick another name.
2. **Sign in** — **Anmelden** on the profile's row opens a terminal running `claude auth login` for that profile. After signing in, refresh (**F5**): the row shows the account instead of **Nicht angemeldet**.
3. **Pick** — **Standard-Profil** sets the one new sessions start with; the composer's **Profil** chip changes it for the next start, and a „Zuletzt“ row restarts with the profile it last used.
4. **What stays in sync** — the folders `skills`, `agents`, `commands`, `rules`, `output-styles`, `workflows`, `agent-memory`, `themes`, `plugins`, `projects`, `file-history`, `plans`, `tasks`, `paste-cache`, `hooks`, `todos` and `session-env` are links into **Standard**'s, so every profile sees the same files at every moment. The rest is synced before every local start, after a profile change, at app start and on **Jetzt synchronisieren** — so a change made in a running session reaches the other profiles with the next start through Traygent: `CLAUDE.md` and `keybindings.json` as whole files (the newer one wins), `history.jsonl` as the union of both, and `settings.json` and the MCP servers, projects and the switches `showSpinnerTree` and `claudeInChromeDefaultEnabled` of `.claude.json` merged setting by setting — what you change in one profile, add or remove, reaches the others, and changes both made to different settings both survive.
5. **What stays per profile** — the login (`.credentials.json`) and the account data of `.claude.json`; in `settings.json` everything that picks an account, routes requests or reports to an organisation (`forceLoginOrgUUID`, `forceLoginMethod`, `apiKeyHelper`, `otelHeadersHelper`, the AWS and Anthropic profile keys, and in `env` the `ANTHROPIC_…`, `CLAUDE_CODE_USE_…`, `AWS_…`, `GOOGLE_…`, `AZURE_…`, `OTEL_…`, `VERTEX_REGION_…`, `CLAUDE_CODE_CLIENT_…` and `CLAUDE_CODE_SKIP_…_AUTH` variables, `CLOUD_ML_REGION`, `CLAUDE_CONFIG_DIR` and `CLAUDE_CODE_OAUTH_TOKEN`); proxy variables (`HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY`) are shared on purpose — they belong to the machine, not the account; in a project the `last…` fields (last session, cost) and `hasUnseenTeamArtifacts`.
6. **Share more** — **Weitere Ordner** and **Weitere Dateien** (shown once there is a second profile) take extra names inside the Claude Code folder, separated by commas or spaces, e.g. `.caveman-sessions` or `orca-state.json`. An extra folder is linked like the built-in ones; an extra file is copied whole, the newer one winning — it is never merged, so two profiles that both changed it keep only the newer edit (the other is in the backup). Plain names only (letters, digits, `.`, `_`, `-`, at most 16 each); a name the sync already handles, the login and state files (and anything named after them, such as `.claude.json.backup`) and the account's caches and logs (`cache`, `statsig`, `telemetry`, `sessions`, `policy-limits.json` and the like) are refused on **Speichern**.

On a profile's first sync **Standard** wins, but nothing is lost: a folder of the profile first hands over what **Standard** lacks, settings, MCP servers and projects only one side has are added to the other, and every file a sync overwrites or takes something from — later too, on either side — is first copied to `.traygent-backup\<time>` in the folder it belongs to (**Standard**'s in `%USERPROFILE%\.claude`). Backups are never cleaned up. Later, where both profiles changed the same setting differently, the newer file wins and the other value stays in the backup. A new profile learns from the others what **Standard** removed, without losing what only it had. A setting, MCP server or project removed in one profile is removed in the others with the next sync; a copied file or a history line deleted in one profile is not, the next sync brings it back. A file a running session just wrote is left for the next sync. A sync that fails shows a warning icon on the row (its tooltip says why) and „Profile konnten nicht vollständig synchronisiert werden.“ after **Starten**; the session starts anyway. Docker sessions ignore the profile and keep their one volume. Removing a profile leaves its folder and login on disk.

## Docker sessions

- Every agent runs in `node:22-trixie` (changeable per agent under **Umgebung**) with `docker run --rm -it --init`; the folder is bind-mounted, all agent state lives in **named volumes** ([ADR-0004](adr/0004-docker-volumes-shared-with-the-batch-starter.md)).
- **Claude Code** uses the same volumes and layout as the `claude/cli-starter.bat` Batch starter of agent-tools (`claude-code-tools`, `claude-code-config`, `/workspace`, the same per-folder project name), so an existing container login carries over in both directions.
- **OpenCode** and **Codex** get Traygent-owned volumes and mount the folder at `/workspace/<project-name>`, so their per-folder session lists stay apart.
- The **first start** pulls the image and installs the CLI into its volume; the agent's Docker status says so beforehand. Codex logs in by device code, or by `OPENAI_API_KEY` when your environment sets it (forwarded by name only).
- Git inside the container gets `safe.directory=*`, `core.autocrlf=input` and your `%USERPROFILE%\.gitconfig` read-only. With **GitHub-Token weitergeben** the session fetches `gh auth token` on the host and forwards it as `GH_TOKEN` by name — it never appears in a command line or the state file.
- Network and WSL paths (`\\server\share`, `\\wsl$\…`) cannot be bind-mounted and are refused.

## Where Traygent keeps its state

Settings, recent sessions and the connected GitHub account's login and id live in one versioned JSON file, `%APPDATA%\io.github.fo0.traygent\state.json`, written atomically ([ADR-0002](adr/0002-versioned-json-state-file.md)). A corrupt file is moved aside as `state.json.corrupt-<timestamp>` and replaced by defaults; a file from a newer Traygent is read but never overwritten. The theme stays in the panel's local storage; the GitHub token stays in the Windows Credential Manager (entry `traygent`, user `github-<id>`).
