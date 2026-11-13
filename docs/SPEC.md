# Traygent — product and IPC contract (v1)

The owner's request, summarized: a Windows tray app on the stack and shell of RepoWatch (`fo0/repo-watch`: Tauri 2 + Svelte 5 + Rust, tray popup, theme light/dark/system, start with Windows, in-app updates, German UI, compact UX rules), whose job is to **start coding-agent sessions as fast as possible** — replacing the Windows Batch starters of `fo0/agent-tools` (`claude/cli-starter.bat` and the OpenCode starter).

Status: shell and agent domain are built (see [ARCHITECTURE.md](ARCHITECTURE.md)); this file stays the product and IPC contract. Planned later: [BACKLOG.md](../BACKLOG.md).

## Look

Same "Ein Blick" style as RepoWatch, slightly different identity: **violet accent** instead of
lime (dark `--accent` around `#b9a5ff`, light theme a darker violet that passes the contrast
tests), and a **robot-head app/tray icon** (rounded square, violet, dark robot head: antenna,
two eyes, mouth) in `src-tauri/icons/source.svg`, generated via `scripts/generate-icons.mjs`.
Brand top left: icon + „Traygent“. Window ~480×760, frameless, tray popup like RepoWatch.

## Domain

- **Agents** (`AgentId`): `claude` (Claude Code CLI), `opencode` (OpenCode V2), `codex` (OpenAI Codex CLI).
- **Environments** (`EnvId`): `local` (CLI on the Windows host), `docker` (CLI in a container,
  workspace bind-mounted), `wsl` (reserved, shown disabled as „bald“ — not implemented).
  `settings.environments` lists the ones the composer offers (default: all three).
- **Permission mode** (`PermissionId`, unified): `ask` (Nachfragen), `auto` (Auto), `bypass`
  (skip all prompts, `danger: true`; labelled the way the CLI names it: „Bypass“ for Claude Code,
  „YOLO“ for OpenCode and Codex). Each agent maps it to its own flags.
- **Session mode** (`SessionMode`, per start, never a default): `new`, `continue` (last session
  of that folder), `resume` (agent's session picker). Agents without support omit the option.
- **Terminal** (`TerminalId`): `windowsTerminal` (wt.exe, default when found) or `console`
  (a new PowerShell console window). The session runs in that window; Traygent does not embed it.
- **Profile** (`AgentProfile`, per agent, local starts only): a sign-in of one CLI in a config
  directory of its own; everything else stays identical to the CLI's own home, the profile
  „Standard“ (id `default`) every agent with `catalog.profiles` has. Claude Code only so far;
  see _Profiles_ below.

## Dashboard (single panel, compact — `agent_docs/ux_rules.md` is binding)

1. AppBar: brand · refresh-detection icon · settings gear.
2. **Neue Session** composer (one card, no prose), directly under the AppBar like RepoWatch's
   search field — no visible section heading; the section is named for assistive tech only and
   the checked agent is the page's focus landing:
   - Agent segmented control (Claude · OpenCode · Codex), each with a small status dot for the
     selected environment (ready / missing). Only the chosen agent shows its name and takes the
     row's room; the others shrink to mark and dot at full contrast (never greyed out — they
     are choices, not disabled ones), the name lives in their tooltip and accessible name
     (owner request, #5).
   - Folder: one field („Ordner oder Repo“) for a folder **or** a GitHub repository + quick list
     of recent folders + a GitHub button (menu of the account's 8 newest-pushed repositories;
     „GitHub verbinden …“ without an account) + browse button (native folder dialog); see
     _Repositories_ below.
     Empty when the panel opens (owner request, #20) — a placeholder shows both forms
     (`C:\Projekte\app oder owner/repo`), never a pre-filled folder.
   - Env segmented control with icons: Lokal (monitor) · Docker (container/whale) · WSL (disabled,
     „bald“) — only the environments shown in Settings → Umgebung („Umgebungen“). With a single
     one the row is hidden and that one is used. A hidden environment (the saved default, a
     recent taken over with „In Composer übernehmen“) falls back to the first shown one that
     starts sessions.
   - Compact option chips for the selected agent: Modell (dropdown, or for agents with free
     model strings an editable combobox whose list opens unfiltered and filters only while
     typing), Denken (thinking/effort dropdown led by signal bars of the chosen level, hidden
     if the agent has none), Freigabe (a
     symbol only — shield, shield with check, bolt — cycling ask → auto → bypass; the label is
     its tooltip and accessible name, bypass shows a warning colour), Session (Neu /
     Fortsetzen / Auswahl). Every dropdown — these chips and the same fields in Settings →
     Agenten and „Bearbeiten …“ — opens the one app-styled list (check mark on the chosen
     value), never the WebView's native select popup. Every agent's chips, Docker's „Ports“ included, keep one row beside
     „Starten“ in the default 480 px window.
   - „Profil“ (last chip before „Starten“, its list opens to the left): only for an agent with
     `profiles`, environment Lokal and at least one extra profile. A 28 px mark only — the
     person icon for „Standard“, else the profile's initial on a round tile; the label is its
     accessible name and tooltip (`Profil: <label> · <account>`). The chosen profile not signed
     in → warning colour and „Nicht angemeldet“ (tooltip and list). Starts at the agent's saved
     „Standard-Profil“, back to it after every start and agent change; a removed profile falls
     back to it. Sent only when it applies and is not `default`.
   - Primary button „Starten“ bottom right; Enter starts. Missing CLI → button becomes
     „Installieren …“ (opens Settings → Agenten for that agent) — never a dead start.
   - Options start from the agent's saved defaults; changing them on the dashboard affects the
     next start only (not the defaults).
3. **Zuletzt** (recent sessions — where RepoWatch lists repositories): one single-line row per
   (folder, agent, env), newest first, max 20 kept, in one bordered list card with dividers
   (RepoWatch's list card). Row: agent mark on a tile — a profile other than „Standard“ sits on
   its corner as its initial on a small round accent tile („Profil <label>“, also in the row's
   tooltip), so it takes no width from the folder name — folder name (primary) + parent path
   (secondary, ellipsized), then fixed columns at the right edge so every icon lines up from
   row to row (`RecentMeta.svelte`; an absent part leaves an empty, `aria-hidden` slot): ports
   chip (the one variable width, first), GitHub mark, env icon (Lokal/Docker), the options as
   symbols — a chosen model
   as a one-letter badge (`optionMarks.ts → modelLetter`: Opus O, Sonnet S, Haiku H, Fable F,
   Opus Plan P; Codex by the word after `gpt-…`: Sol S, Astra A, Terra T, Luna L; else the
   first letter of the first word after the last `/`, e.g. `google/gemini-2.5-flash` G), the
   thinking level as five signal bars (Niedrig 1 … Maximal 5, „Standard“ all dim) wherever the
   agent sends one, the permission icon (bypass a bolt in the warning colour) — each with its German label as
   accessible name, relative time (right-aligned, fixed width), „⋯“ menu (In Composer übernehmen, Bearbeiten …, Ordner öffnen,
   Pfad kopieren, Entfernen with 8 s „Rückgängig“) and ▶ at the far right. A click anywhere on the row (or Enter / Space on
   ▶) restarts with exactly those settings — once per press: the rest of a double-click, a
   held key and a second start of the same row within 1.5 s after it started are ignored;
   „⋯“ and its menu never start, nor does a press that only closes a popup (a row menu, the
   „Ports“ popover, the folder suggestions). Hiding an environment is a display preference, not
   a ban: its recents keep their environment and ▶ restarts them unchanged. „Bearbeiten …“
   (#3) opens a staged dialog with the row's model, thinking level, permission, extra
   arguments and (Docker) ports; „Speichern“ stores them on the entry without a start
   (`update_recent`, validated like a start; the dialog also applies the npm-shim argument rule) — place, start time and count stay, the next ▶
   sends them. The agent, environment and folder define the entry and are not editable.
   A row's profile: ▶ restarts with it (one removed since is refused natively, „Profil nicht
   gefunden.“); „In Composer übernehmen“ takes it over, or the saved „Standard-Profil“ when it
   is gone. A start of the same folder with another profile updates the row's profile.
4. Footer: Docker state („Docker 27.5 bereit“ / „Docker nicht erreichbar“, only while Docker is
   shown) · „Stand hh:mm“.

## Settings dialog (staged: „Speichern“ + „Abbrechen“; tabs)

- **Agenten**: per agent a section: local status (version or „Nicht installiert“), Docker
  readiness (only while Docker is shown), best-practice Windows install command (copy button + „Installieren“ = opens a
  terminal running it, then re-detects on demand), update command; defaults: model, effort,
  permission, extra CLI args (free text, validated). For an agent with `profiles` a „Profile“
  block (info tip „Eigene Anmeldung je Profil, alles andere wird abgeglichen. Weitere Ordner und
  Dateien werden wie die eingebauten abgeglichen.“): „Jetzt
  synchronisieren“ beside the heading (immediate, `sync_profiles`), „Standard-Profil“ (staged),
  one row per profile — „Standard“ first — with its name, the account or „Nicht angemeldet“
  (tooltip: account, organisation, folder, „Abgeglichen <time>“, the sync error), a warning
  icon after a failed sync, „Anmelden“ (immediate, `profile_login`; „Erst speichern“ while the
  profile is only staged) and a remove button (staged; none for „Standard“); then „Neues
  Profil“ + „+“ / Enter, whose tooltip shows the folder the name gives. Inline errors: „Name
  fehlt.“, „Höchstens 24 Zeichen.“, „Keine Steuerzeichen.“, „„<Name>“ ist reserviert.“,
  „Mindestens ein Buchstabe oder eine Ziffer.“, „Profil gibt es schon.“, „Höchstens 8
  Profile.“ With at least one extra profile staged, „Weitere Ordner“ and „Weitere Dateien“
  (`profileShare`, staged; names split at commas and whitespace, repeats once, tooltip „Durch
  Komma oder Leerzeichen getrennt“, placeholders `.caveman-sessions` / `orca-state.json`) with
  the native errors of _Profiles → Extra entries_ inline, the first problem per field. Removing
  the last extra profile hides them; a list that is then invalid goes back to its saved value.
- **Umgebung**: „Umgebungen“ — one checkbox per environment the composer shows (Lokal / Docker /
  WSL, all on by default; the last of Lokal and Docker cannot be turned off and says why in its
  tooltip), default environment (one of the shown ones, WSL disabled; it moves to the first shown
  one when its own is hidden, the row hides with a single one), default agent, terminal
  (Windows Terminal / Konsole; with Windows Terminal, `sharedWindow` (default off) opens every
  start as a tab of one window named `traygent`), Docker: image per agent (defaults from research), pass host gh
  token (off by default). Rust stores the list each once in `local` · `docker` · `wsl` order and
  refuses one without Lokal or Docker („Mindestens eine Umgebung muss aktiv sein.“, „WSL ist noch
  nicht verfügbar.“) or without the default („Die Standard-Umgebung ist deaktiviert.“); a
  `state.json` without the field shows all three, an unknown id is dropped. While the staged list
  hides Docker, the Docker section (images, default ports, gh token) and the Agenten tab's Docker
  readiness are hidden too, their values kept; unticking Docker resets each invalid Docker field to
  its saved value, valid edits stay staged; the default ports count as one field, so a list with a
  conflict resets as a whole
  ([ADR-0007](adr/0007-persisted-environment-visibility.md)).
- **GitHub**: the one account (`login · Token` / `login · gh CLI`, „Trennen“ with inline
  confirm, „Aktualisieren“ for the listing) or, without one, two ways to connect: „Lokale
  Anmeldung prüfen“ (lists gh sign-ins, one consent line, „Übernehmen“) and a masked token field
  with „Verbinden“. Account actions apply at once; the clone root („Klon-Ordner“, placeholder
  the default) is staged like every setting.
- **App**: theme (System/Hell/Dunkel), Mit Windows starten.
- **Updates**: in-app updates — „Nach Updates suchen“, changelog, „Installieren“ — as in RepoWatch.
  Below it „Agenten“: the staged checkbox „Automatisch aktualisieren“ (`settings.agentUpdates`,
  default off), „Jetzt aktualisieren“ and one line per agent with its last result; see
  _Agent updates_ below.

Later (BACKLOG, not now): see [BACKLOG.md](../BACKLOG.md).

## IPC contract (camelCase JSON; `src/lib/types.ts` ↔ `src-tauri/src/models.rs`)

`src/lib/types.ts` is the contract; Rust mirrors it field by field. A Rust test writes the
default native snapshot to `fixtures/snapshot.default.json`, and a panel test checks that it
satisfies `Snapshot`; the same way `fixtures/profile-share-names.json` keeps the panel's
extra-name rules equal to Rust's (_Profiles → Extra entries_). Per-agent flags, install commands and Docker recipes:
`docs/AGENT_RECIPES.md`.

Detection runs off the UI thread at startup and on `refresh_detection`; while it runs the
snapshot has `detecting: true`, and the shell emits `traygent:snapshot` with the finished
snapshot. `refresh_detection` returns at once with `detecting: true`.

Profiles in the contract (`profileTypes.ts` ↔ `models/profiles.rs`): `AgentCatalogEntry.profiles`
(Claude Code only), `AppSettings.profiles` (`Record<AgentId, AgentProfile[]>`, `{ id, label }`),
`AppSettings.agentProfile` (`Record<AgentId, string>`, `default` or a listed id),
`AppSettings.profileShare` (`Record<AgentId, ProfileShare>`, `{ dirs: string[], files: string[] }`,
empty lists by default and for an agent without `profiles`),
`LaunchRequest.profile` and `RecentSession.profile` (optional, absent for `default`),
`Snapshot.profiles` (`ProfileStatus[]`: `agent`, `id`, `label`, `dir`, `signedIn`, optional
`account`, `organization`, `syncedAt`, `syncError`) and `LaunchOutcome.warning` (optional,
German: the session started, the profile sync before it was incomplete).

Commands (Rust `#[tauri::command]`, German `Err` strings; panel calls only via `src/lib/ipc.ts`):

| command                            | args                                                                                             | result                                                        |
| ---------------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------- |
| `snapshot`                         | –                                                                                                | `Snapshot` (cached detection)                                 |
| `refresh_detection`                | –                                                                                                | `Snapshot` (re-probes CLIs + Docker)                          |
| `save_settings`                    | `{ settings }`                                                                                   | `Snapshot` (validated; invalid → Err)                         |
| `launch_session`                   | `{ request }`                                                                                    | `LaunchOutcome` (validates, opens terminal, records recent)   |
| `remove_recent`                    | `{ id }`                                                                                         | `Snapshot`                                                    |
| `restore_recent`                   | `{ recent }`                                                                                     | `Snapshot` (undo of remove)                                   |
| `update_recent`                    | `{ id, options, ports }`                                                                         | `Snapshot` (edit, no start)                                   |
| `pick_folder`                      | `{ initial?: string }`                                                                           | `string \| null` (native folder dialog)                       |
| `install_agent`                    | `{ agent }`                                                                                      | `void` (opens a terminal running the catalog install command) |
| `open_folder`                      | `{ path }`                                                                                       | `void` (Explorer)                                             |
| `update_agents`                    | –                                                                                                | `Snapshot` (starts an agent-CLI update run, events follow)    |
| `github_connect_token`             | `{ token }`                                                                                      | `Snapshot` (verifies `/user`, vaults the token, lists repos)  |
| `github_local_accounts`            | –                                                                                                | `LocalAccounts` (gh sign-ins; reads no token)                 |
| `github_import_local`              | `{ login }`                                                                                      | `Snapshot` (explicit consent: reads that gh token, as above)  |
| `github_disconnect`                | –                                                                                                | `Snapshot` (deletes the vault copy; gh stays signed in)       |
| `github_refresh_repos`             | –                                                                                                | `Snapshot` (returns with `github.loading`, event follows)     |
| `sync_profiles`                    | –                                                                                                | `Snapshot` (syncs every profile now, statuses in `profiles`)  |
| `profile_login`                    | `{ agent, profile }`                                                                             | `void` (opens the terminal that signs that profile in)        |
| shell commands kept from RepoWatch | autostart_status, set_autostart, open_startup_settings, check_update, install_update, hide_panel | unchanged                                                     |

## Repositories (GitHub, [ADR-0005](adr/0005-start-from-a-github-repository.md))

- The folder field classifies its trimmed text (panel live, Rust authoritative): an absolute
  Windows path → folder; `[http[s]://][www.]github.com/<owner>/<repo>[.git][/…|?…|#…]`
  (case-insensitive host, no whitespace), `git@github.com:<owner>/<repo>[.git]` or
  `<owner>/<repo>` (exactly one `/`) → repository; empty → nothing;
  anything else → search. Owner: 1–39 of `A-Za-z0-9-`, no leading/trailing `-`; name: 1–100 of
  `A-Za-z0-9._-`, not `.`/`..`, a `.git` suffix dropped. Test vectors: `fixtures/targets.json`.
- Search: every whitespace-separated word must occur (case-insensitive) in `owner/name`; at most
  8 suggestions from `github.repos`, newest push first, in a listbox under the field (↑/↓, Enter
  picks, Esc closes). A picked suggestion becomes `owner/name`. Without an account the list
  holds one entry „GitHub verbinden …“ (Settings → GitHub). A repository typed or pasted needs
  no account.
- Start: `repo` set, `folder` `''` (composer) or the recent's folder (▶ on a „Zuletzt“ row). A
  given folder (validated like any folder) that exists is started as is, without a clone, so a
  recent keeps its clone after the clone root changed (#40). Otherwise Rust derives
  `<cloneRoot>\<owner>\<name>` (validated like any folder; `cloneRoot` `''` →
  `github.defaultCloneRoot` = `%USERPROFILE%\Traygent`, none → „Kein Klon-Ordner verfügbar.
  …“). Existing folder → started as is. Otherwise Rust builds the plan, then creates the clone
  root, runs the terminal there, and the script runs `& <git> 'clone' '--' 'https://github.com/<owner>/<name>.git' '<folder>'`
  and stops (`throw` „[traygent] Das Repository konnte nicht geklont werden. …“) on a non-zero
  exit before `Set-Location`. Git missing → „Git ist nicht installiert.“. The token is never
  used for the clone. A Docker start clones on the host, then bind-mounts the folder as usual.
- Recents: an entry started from a repository keeps `repo` (validated on load, an invalid one
  dropped) and shows a repository icon; ▶ starts in the entry's folder while it exists,
  otherwise from the repository (clone into the current Klon-Ordner, ADR-0006), „In Composer
  übernehmen“ puts `owner/name` into the field.
- Listing: `GET /user/repos?affiliation=owner,collaborator,organization_member&sort=pushed&per_page=100`,
  at most 10 pages, read at startup, after connecting, on `github_refresh_repos` and when the
  panel focuses the field with a listing older than 15 minutes (at most once per 15 minutes).
  At most 1000 entries, malformed ones skipped, duplicates dropped, `Link: rel="next"` followed
  on the same path only. HTTPS only, no redirects, 10 s connect / 30 s request timeout, body
  ≤ 4 MiB. A 401 keeps the account and the last repositories and sets `github.error`
  („GitHub-Token ungültig oder abgelaufen.“); 429 or an exhausted rate limit → „GitHub begrenzt
  gerade die Anfragen. …“.
- Account: one; `state.json` holds `github: { login, id, source }`, the token lives in the OS
  vault (service `traygent`, user `github-<id>`). gh import follows RepoWatch `docs/LOCAL_AUTH.md`
  (version-aware flags, `auth token --hostname github.com --user <login>` plus `--no-refresh`
  when known, minimal child env without token variables, stdout/stderr capped at 64 KiB, 20 s
  timeout, `/user` must match the chosen login). Without the `vault` feature every vault call
  fails closed („Windows-Tresor nicht verfügbar.“).

## Agent updates ([ADR-0008](adr/0008-background-updates-of-the-agent-clis.md))

- With `settings.agentUpdates` on, Rust runs each locally installed CLI's own update command
  (`claude update`, `opencode upgrade`, `codex update`) on its resolved path — hidden, closed
  stdin, 10-minute timeout: about a minute after the app started, then every 4 hours.
  `update_agents` („Jetzt aktualisieren“) starts a run at once, whatever the setting; a second
  run while one is going is refused („Agenten werden bereits aktualisiert.“), as is one while a
  detection runs or before the first finished („Die Agenten werden noch gesucht. …“; the
  schedule retries a minute later) and one without a ready local CLI („Kein lokaler Agent
  installiert.“). A run takes the agents one after the other and ends the 4-hour wait anew (the
  schedule counts a try without a ready local CLI as a run); an
  update command that runs into its timeout is ended with its whole process tree.
- An agent is skipped (`inUse`) while a process of its CLI runs: its executable, or a `node` /
  `bun` process whose command line names its package — sessions started outside Traygent count;
  the Claude desktop app (`Claude.exe` in its own install folders) does not, and a `node` / `bun`
  process with an unreadable command line counts for every agent.
  The process list comes from `Get-CimInstance Win32_Process` through Windows PowerShell; a list
  that cannot be read, times out or is incomplete skips every agent (fail closed).
- `Snapshot.agentUpdates` holds one `AgentUpdateStatus` per agent that had a run since the app
  started, in catalog order: `running`, `updated` (`from` → `version`), `current`, `inUse`,
  `failed`, each with its time `at`; `Snapshot.agentUpdatesRunning` is true while a run is in
  progress. `update_agents` returns before the first agent turns `running`. Every change emits
  `traygent:snapshot`; a detection follows every run (one already running reruns), so versions
  and paths refresh. Nothing is persisted.
- A local `launch_session` of an agent in `running` is refused („<Agent> wird gerade
  aktualisiert.“); Docker starts are not affected, and the CLI in a container keeps its own
  update path.

## Profiles ([ADR-0009](adr/0009-agent-profiles-with-linked-config-directories.md), [ADR-0010](adr/0010-three-way-merge-of-the-profiles-settings-and-state-file.md))

- **Recipe** (`profiles.rs → Recipe`, data per agent): Claude Code only — variable
  `CLAUDE_CONFIG_DIR`, login `<cli> auth login`. OpenCode and Codex have none: any extra profile
  for them is refused („<Agent> unterstützt keine Profile.“).
- **Directories** (`profiles.rs → Homes`, the only path builder; from the user profile, which
  must be absolute and fully expanded): „Standard“ = `%USERPROFILE%\.claude` with its state file
  beside it (`%USERPROFILE%\.claude.json`); profile `<id>` = `%USERPROFILE%\.claude-<id>` with
  `.claude.json` inside. A name „Team“ gives id `team`, so an existing `.claude-team` is adopted
  with its login.
- **Rules** (Rust authoritative, panel mirrors them): id `^[a-z0-9][a-z0-9-]{0,23}$`, not
  `default` (the panel derives it from the name: lower case, ä ö ü ß spelled out as ae oe ue
  ss, other diacritics dropped, every other run of characters one `-`, trimmed, at most 24 —
  „Büro“ gives `buero`); label trimmed, 1–24 characters, no control characters, not
  „Standard“; ids and labels (ignoring case) unique per agent; at most 8 extra profiles per
  agent. Errors: „Profil-ID für <Agent> ist ungültig: nur a–z, 0–9 und „-“, höchstens 24
  Zeichen, nicht „default“.“, „Profilname für <Agent> fehlt.“, „Profilname für <Agent> ist zu
  lang (höchstens 24 Zeichen).“, „Profilname für <Agent> enthält Steuerzeichen.“, „Profilname
  „Standard“ ist für das Standard-Profil reserviert.“, „Profil-ID „<id>“ für <Agent> ist doppelt
  vergeben.“, „Profilname „<Name>“ für <Agent> ist doppelt vergeben.“, „Höchstens 8 Profile für
  <Agent> möglich.“. An `agentProfile` that is not listed becomes `default`, so removing a
  profile never blocks the save; on load every entry that would not validate is dropped, the
  rest kept. Removing a profile leaves its directory, login included, on disk.
- **Start** (local only; Docker normalizes `profile` away and keeps its one volume, ADR-0004):
  absent, blank or `default` = „Standard“; any other id must be listed („Profil nicht
  gefunden.“). No user profile → „Der Profilordner ist nicht verfügbar: kein Benutzerordner
  gefunden.“. The script sets, after `Set-Location` and before the agent call,
  `$env:CLAUDE_CONFIG_DIR='<profile dir>'` (Windows-absolute, else „Der Profilordner ist kein
  absoluter Windows-Pfad.“); for „Standard“ it emits
  `Remove-Item Env:CLAUDE_CONFIG_DIR -ErrorAction SilentlyContinue` while the agent has at
  least one extra profile, so an inherited value cannot move the default home; without extra
  profiles there is no line. Before the spawn every profile of that agent is synced, outside
  the service lock (plan under the lock → run → commit → spawn); right before the spawn the
  start is checked again — the agent not updating („<Agent> wird gerade aktualisiert.“), the
  profile still listed („Profil nicht gefunden.“). A failed or crashed sync starts the
  session anyway with `LaunchOutcome.warning` = „Profile konnten nicht vollständig
  synchronisiert werden.“ (shown after „<Agent> gestartet.“ in the warning tone; the detail
  stays in each profile's `syncError`). Only the requested profile's directory refuses the
  start: „Der Profilordner konnte nicht angelegt werden.“, or „Der Profilordner gehört nicht zu
  Claude Code.“ for a foreign one (below).
- **Extra entries** (`settings.profileShare`, `settings/share.rs`, names checked by
  `profiles/share.rs → check_name`):
  per agent `dirs` (linked like the recipe's directories) and `files` (whole-file like
  `CLAUDE.md`), plain names inside the config directory. Name: `^[A-Za-z0-9._-]{1,64}$`, no
  trailing dot (so not `.` / `..`), no Windows device name with or without an extension (`con`,
  `prn`, `aux`, `nul`, `com0`–`com9`, `lpt0`–`lpt9`), not starting with `.traygent`,
  `.claude.json` or `.credentials.json` (copies such as `.claude.json.backup` carry the same
  identity), none of the recipe's `never_shared` names (account policy, caches and machine state:
  `policy-limits.json`, `policy-limits.json.stamp.json`, `remote-settings.json`, `cache`,
  `statsig`, `telemetry`, `backups`, `debug`, `shell-snapshots`, `ide`, `local`, `sessions`,
  `jobs`, `daemon`) — all „ist ungültig“ — and none the recipe handles itself (its shared
  directories, `settings.json`, `CLAUDE.md`, `keybindings.json`, `history.jsonl`: „wird bereits
  abgeglichen“); all ignoring ASCII case; unique ignoring case across both lists; at most 16 per
  list. The lists behind the rule are written by a Rust test to `fixtures/profile-share-names.json`
  (`share.rs → name_lists`), and the panel's copy (`profileShare.ts`) is tested against it. Saving trims, drops empty names and exact repeats
  within a list, then refuses: „<Agent> unterstützt keine Profile.“ (an entry for an agent
  without a recipe), „Höchstens 16 weitere Ordner für <Agent> möglich.“, „Höchstens 16 weitere
  Dateien für <Agent> möglich.“, „Name „<name>“ für <Agent> ist ungültig.“, „Name „<name>“ für
  <Agent> wird bereits abgeglichen.“, „Name „<name>“ für <Agent> ist doppelt vergeben.“. On load
  every name that would not validate is dropped (folders first), the rest kept; a `state.json`
  without the field shares nothing extra. The sync checks every name again when it runs: one that
  fails (also past 16, or a repeat) is never joined into a path and gives each profile the error
  „<name>: ungültiger Name, wird nicht abgeglichen“ (control characters dropped, at most 64
  characters).
- **Triggers:** the first detection after the app started (later detections only read the
  statuses), `save_settings` when `profiles` or `profileShare` changed (otherwise a re-read),
  `sync_profiles`, a local start (that agent only) and a background sync every 15 minutes
  (`PROFILE_SYNC_INTERVAL`, fixed, no setting): the `traygent-profile-sync` thread checks once a
  minute and syncs every profile once 15 minutes have passed since the last sync of any kind
  finished (before the first one, since the app started), only while some agent has an extra
  profile and the user profile is known, also during an agent update (a sync never runs the CLI).
  It is silent apart from each profile's `syncedAt` / `syncError`, which reach an open panel as
  a `traygent:snapshot` event; a run that broke down also waits the full interval. An agent
  without an extra profile is never touched. One sync runs at a time; every run's file system
  work happens without the service lock.
- **Profile directory:** created when missing; a new or empty one gets the initial marker
  `{"version":1,"done":[]}` before anything is linked into it. An existing, non-empty one is
  adopted only when it holds `.claude.json`, `.credentials.json` or the marker, or nothing but
  `.traygent-base` and links into „Standard“'s shared directories, the extra ones included (a sync
  stopped before its marker); otherwise it belongs to something else (`.claude-mem`,
  `.claude-backup`) and nothing in it is touched: „Profilordner gehört nicht zu Claude Code“. One
  that resolves to „Standard“'s: „Profilordner ist der Ordner des Standard-Profils“; a comparison
  without an answer: „Profilordner konnte nicht geprüft werden“; a file in its place or a failed
  create: „Profilordner konnte nicht angelegt werden“. No rule runs then.
- **Sync** of each profile with „Standard“, in this order:
  1. Shared directories `skills`, `agents`, `commands`, `rules`, `output-styles`, `workflows`,
     `agent-memory`, `themes`, `plugins`, `projects`, `file-history`, `plans`, `tasks`,
     `paste-cache`, `hooks`, `todos`, `session-env` and the extra `dirs` become directory links
     into „Standard“'s directory of that name (created there when missing): a junction on
     Windows — all missing ones in one hidden call of an absolute `powershell.exe` (never a bare
     name; without one nothing is linked) (`New-Item -ItemType Junction`, link/target lines
     Base64 in the env var `TRAYGENT_LINKS`, never on the command line, 20 s timeout) — a
     symbolic link elsewhere. In the profile: a link to the target stays; an empty directory is
     removed and linked; a non-empty one first hands every entry „Standard“ lacks over
     (recursively, no file overwritten, links not followed, file times and the read-only flag
     kept), then moves to the backup and is linked. Each new link is checked; a link that failed
     gives the profile its directory back (from the backup, or the removed empty one anew).
     Errors: „<name>: Verknüpfung zeigt auf ein anderes Ziel“ (also a dangling link), „<name>:
     ist kein Ordner“, „<name>: ist im Standard-Profil kein Ordner“, „<name>: Verknüpfung konnte
     nicht angelegt werden“, „<name>: Inhalt konnte nicht übernommen werden“, „<name>: Zugriff
     fehlgeschlagen“.
  2. `settings.json`: a three-way merge (below) of its shared part. Shared is the top-level
     object without the own keys `apiKeyHelper`, `forceLoginOrgUUID`, `forceLoginMethod`,
     `awsAuthRefresh`, `awsCredentialExport`, `anthropicProfile`, `anthropicProfiles`,
     `otelHeadersHelper` and without the own variables of `env`: names starting with
     `ANTHROPIC_`, `CLAUDE_CODE_USE_`, `AWS_`, `GOOGLE_`, `AZURE_`, `OTEL_` (organisation-bound
     telemetry), `VERTEX_REGION_` or `CLAUDE_CODE_CLIENT_` (the mTLS client certificate and key);
     `CLAUDE_CONFIG_DIR`, `CLAUDE_CODE_OAUTH_TOKEN`, `CLOUD_ML_REGION`; and
     `CLAUDE_CODE_SKIP_<…>_AUTH` (ASCII case ignored; the recipe's `SettingsRule`). `HTTP_PROXY`,
     `HTTPS_PROXY` and `NO_PROXY` stay shared on purpose: they describe the machine's network,
     not an account. Own keys and variables never leave their file, never
     reach the base and never make two sides differ; `env` is shared only while it holds a shared
     variable. Lists of strings below `permissions.` and `autoMode.` merge as sets: an item one
     side added stays, one a side removed since the base goes; „Standard“'s order, then the
     profile's new items. Each side becomes the merged shared part plus its own keys and
     variables, in its own key order at every depth (new keys appended); an `env` object it had
     stays one, even empty. Written as the CLI writes it — two-space indent, `\n`, a final line
     break, no byte order mark — and only when its content changes; numbers keep the token the
     file wrote. A missing file is created with the shared part. A file of only whitespace (the
     CLI is writing it) skips the rule this run, neither done nor failed. Errors:
     „settings.json: ist kein lesbares JSON-Objekt und bleibt unverändert“ (also nesting deeper
     than 128), „settings.json: „env“ ist kein Objekt und bleibt unverändert“ (a shared `env`
     that is no object while a side has own variables), „settings.json: ist zu groß“ (over 8 MiB)
     and those of every merged write.
  3. `CLAUDE.md`, `keybindings.json` and the extra `files` (one rule each, whole file): equal
     content writes nothing; a file on one side is copied to the other; differing files — until
     the rule's first sync succeeded „Standard“ wins, later the newer modification time (a tie:
     „Standard“). The losing file is copied to its own side's backup before every overwrite,
     whichever side it is. A copy keeps its source's file time (set before the rename); writes
     are temp file + rename; a
     link as source is followed, a link as target never replaced (checked before the backup);
     errors as `<name>: …` — „ist eine Verknüpfung und wird nicht überschrieben“, „ist zu groß“
     (over 8 MiB), „ist keine Datei“, „Zugriff fehlgeschlagen“.
  4. `history.jsonl`: both sides become the union of their lines (exact duplicates dropped,
     „Standard“'s order kept, the profile's other lines placed by their numeric `timestamp`). A
     side whose size or file time changed since the read is not written („history.jsonl: wurde
     gerade geändert, nächster Versuch beim nächsten Abgleich“), so a line a session just
     appended survives; over 64 MiB left alone. No backup: nothing is lost.
  5. `.claude.json`: a three-way merge (below) of the shared subset — `mcpServers` and
     `projects`, merged entry by entry and, inside an entry, field by field, plus
     `showSpinnerTree` and `claudeInChromeDefaultEnabled`, one value each. Inside a `projects`
     entry the fields whose name starts with `last` or is `hasUnseenTeamArtifacts` (ASCII case
     ignored) stay with their side: they never travel, never reach the base and never make two
     entries differ; a rewritten entry keeps its side's own. Every other key — identity,
     onboarding state, caches — keeps its bytes and place and is never read into the base; only a
     changed key is written anew, an unchanged entry keeps its bytes, the layout (indent, line
     ends, final line break, a byte order mark) is kept; a missing file is created with the shared
     subset only. `mcpServers` or `projects` holding anything but an object on either side (e.g.
     `null`) is skipped: left as it is on both sides and kept out of the base, while the other
     keys merge and the rule counts as synced; the profile still reports „.claude.json: „<key>“
     ist kein Objekt und bleibt unverändert“ (one entry per such key), so `syncedAt` does not
     move while it stands. Errors: „.claude.json: ist kein lesbares JSON und bleibt unverändert“,
     „.claude.json: ist zu groß“ (over 64 MiB) and those of every merged write. Upgrade: a pair
     without a value base whose marker still has the name-only `base` of an earlier version uses it once (not on the rule's first sync) — each
     recorded entry only one side still holds counts as removed on the other side; a key with 2
     or more recorded names of which one side holds none gets no base (reset).
  6. When one profile changed „Standard“ and there are several, steps 2–5 run once more, so the
     change reaches the others in the same run.
- **Three-way merge** (`settings.json`, `.claude.json`; `profiles/merge.rs`, `pair.rs`): both
  shared parts are compared as values — key order, formatting and two spellings of one number
  (`1.5` / `1.50`) never differ — against the pair's base, the shared part both sides held after
  the rule's last successful sync. Objects merge key by key at every depth: a key only one side
  has stays, unless the base holds it with that very value (the other side removed it: it goes);
  a value only one side changed since the base is taken from that side; a value both changed
  differently is a conflict — decided whole for that value by the newer file (a tie:
  „Standard“). On the rule's first sync (until it succeeded once the pair's base is ignored)
  „Standard“ wins every conflict. Without a base — first sync, a missing or unreadable base, a
  reset — nothing is removed, and a rule already in `done` (a profile synced by a version from
  before the base) lets the newer file win. A pair without a base of its own takes the base
  another profile of the agent held at the start of the run as a seed; „Standard“ then wins every
  conflict, and the seed only tells what „Standard“ removed. It is read through containers —
  objects whose children are independent: the top level, in `settings.json` also `env`,
  `permissions` and `autoMode`, in `.claude.json` `mcpServers` and `projects` (their entries are
  units). Of a key only the profile has that the seed knows: a container is reduced child by child
  and goes only when empty; a string set loses the items the seed knows; anything else — a unit
  object such as `statusLine`, an MCP server, a project entry, a plain value — is dropped when it
  equals the seed's value and kept whole when the profile changed it (it then returns to every
  home). A key both sides hold that is a non-container object gets both sides' fields, „Standard“
  winning a differing one. What the seed never saw stays, so a new profile neither brings back
  what „Standard“ removed and it still holds unchanged, nor loses its own additions. A seed of which „Standard“ holds none of 2 or more keys tells nothing and is not used
  (`settings.json`: its top-level keys; `.claude.json`: per entry key, its entries). Reset guard:
  in `settings.json`, a base of 2 or more keys of which one side holds none merges without base
  and seed this run; in `.claude.json`, the same per entry key (2 or more entries in the base,
  none on one side) unites that key this run. While one file is missing nothing is removed;
  neither present → nothing to do.
- **Merged writes** (`pair.rs → commit`): everything is checked before the first write — a link
  is never written through („<file>: ist eine Verknüpfung und wird nicht überschrieben“), a side
  whose size or file time changed since the read stops the rule („<file>: wurde gerade geändert,
  nächster Versuch beim nächsten Abgleich“; neither side written, base unchanged, the next sync
  merges the new content). A side goes to its own home's backup first on the rule's first sync,
  when the merge takes something from it (a key, list item, entry or field, or an object or list
  replaced by something else) and when it lost a conflict. Writes are temp file + rename, „Standard“
  first; a written file gets the newest file time of the two sides read, never the sync's moment,
  set on the temp file before the rename (a session writing right after keeps its newer time).
  The base is written last, only when it changed: „<file>: Abgleichsstand konnte nicht
  gespeichert werden“ when that fails.
- **Base** `<profile>\.traygent-base\settings.json` and `<profile>\.traygent-base\.claude.json`:
  the merged shared part as values (two-space JSON) — MCP server commands, arguments and `env`
  included, never an own key, own variable, own project field or anything of the sign-in. It lives
  in the profile's directory only (never in „Standard“'s), outside every shared directory: nothing
  links, syncs or backs it up. `.traygent-base` must be a real directory (a link there reads as no
  base and refuses the write); a base over 8 MiB is not read, and a shared part over 8 MiB is
  recorded as `{}` (the next sync unites, removes nothing).
- **Marker** `<profile>\.traygent-sync.json`, compact on one line:
  `{"version":1,"syncedAt":"<RFC 3339>","done":["settings.json","CLAUDE.md","keybindings.json",…,".claude.json"]}`.
  `done` lists the rules (`settings.json`, each whole-file rule by name — extra files included —
  and `.claude.json`, also with a skipped key) whose first sync succeeded; a rule that failed stays a first sync in the
  next run. `syncedAt` moves only after a sync without errors. A name-only `base`
  (`{"mcpServers":[…],"projects":[…]}`) from an earlier version is kept as read until
  `.claude.json` syncs without an error, then dropped; it is never recorded anew. Written whenever
  it changed; an unreadable marker counts as present (all rules first).
- **Backup** `<home>\.traygent-backup\<UTC yyyymmddThhmmssZ>[-<n>]\<name>[.<n>]`, one folder per
  run and home, created on first use: a profile's losers in the profile's directory, „Standard“'s
  in `%USERPROFILE%\.claude` (also for `.claude.json`, which lives beside it), so no identity
  crosses profiles. Traygent never deletes a backup.
- **Never touched:** `.credentials.json` (only its existence and size make `signedIn`); the own
  keys and variables of `settings.json` and the own project fields of `.claude.json` stay in their
  file; every other key of `.claude.json` (`oauthAccount.emailAddress` /
  `organizationName` are read for `account` / `organization` only, control characters dropped, at
  most 128 characters, a byte order mark tolerated); any other file of either directory. Nothing
  is deleted but an empty directory that makes room for a link. Other errors: „Benutzerordner
  nicht verfügbar“, „Ordner des Standard-Profils konnte nicht angelegt werden“,
  „.traygent-sync.json: Zugriff fehlgeschlagen“.
- **Statuses:** `Snapshot.profiles` per agent with `profiles`, „Standard“ first, then the
  settings' order; `syncError` = the last sync's errors joined by `·` (at most 400 characters). In
  memory only; read at every detection, after a sync and a settings save.
- **Sign-in:** `profile_login { agent, profile }` („Standard“ included, only on a click) creates
  the profile directory and opens a terminal in `%USERPROFILE%` (else `C:\`) titled „<Agent> ·
  <Label> anmelden“ running the env line (for „Standard“ always the `Remove-Item`) and
  `& '<cli>' 'auth' 'login'`; it stays open like the install terminal. Refused: „<Agent> wird
  gerade aktualisiert.“, „<Agent> ist nicht installiert.“, „Profil nicht gefunden.“, and „Der
  Profilordner gehört nicht zu Claude Code.“ for an existing foreign directory (the rule above,
  with the agent's extra directories; also when that cannot be checked) — a sign-in never writes
  credentials into it. The status follows with the next detection, sync or settings save.

## Ports (Docker sessions)

- A Docker session can publish ports (`docker run -p`). The composer shows a „Ports“ chip only
  for Docker — the ports glyph, then the count and an exposing IP once ports are chosen; the
  word „Ports“ is its accessible name: checkboxes for the presets (`Snapshot.portPresets`, sent by Rust) plus custom
  entries; Settings → Umgebung holds the default list (`docker.ports`) every new Docker start
  begins with. A recent session stores and restarts with its ports. WSL needs none (WSL 2
  forwards `localhost`).
- Presets: 3000 Next.js · Node, 5173 Vite, 4200 Angular, 8000 Django · FastAPI, 8080 Spring ·
  HTTP, 5000 Flask · ASP.NET, 4321 Astro, 6006 Storybook.
- Spec grammar (Docker's): `PORT` | `HOST:CONTAINER` | `IP:HOST:CONTAINER`, each port a number
  1–65535 or a range `A-B` of at most 100 ports (host and container ranges of equal length), optional `/tcp` or
  `/udp`; IP is IPv4. **Normalized form binds to `127.0.0.1`** unless the user typed an IP
  explicitly (`3000` → `127.0.0.1:3000:3000`, `8080:80` → `127.0.0.1:8080:80`), so a dev server
  is never exposed to the LAN by accident. Both sides validate; Rust normalizes and is the
  authority. Duplicate host ports (same protocol, overlapping ranges) are refused; at most 20
  entries.
- The service inside the container must listen on `0.0.0.0` (tooltip hint, no prose).

## Launch rules (Rust, pure builder + thin Windows spawn adapter)

- Build an argv `Vec<String>` per (agent, env, options, session). Never build a cmd.exe string.
  The terminal runs `powershell.exe -NoLogo -NoExit -EncodedCommand <Base64 of the UTF-16LE
script>`; inside the script every argument is a PowerShell single-quoted literal (`'` and the
  typographic single quotes doubled) invoked with `&`. The encoded form carries no `;`, `%`,
  quote or space, so wt.exe never splits or expands it (ADR-0003).
- Folder must exist, be absolute, be a directory; reject control chars, device paths (`\\?\`, `\\.\`), alternate streams and reserved names. Extra args: split on
  whitespace, reject control chars and quotes; each becomes its own literal.
- Window title: `<agent glyph> <folder name>` (Claude Code ✳, OpenCode ▣, Codex ❯; a repository
  start's folder name is the repository name). Windows Terminal tabs get `--title` plus
  `--suppressApplicationTitle`; Claude Code also gets `CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1`
  (locally and in the container), so the CLI cannot replace it.
- Profiles (local, an agent with a recipe): one `CLAUDE_CONFIG_DIR` line — set to the profile's
  directory, or removed for „Standard“ while extra profiles exist — before the agent call; see
  _Profiles_.
- Closing: `-NoExit` keeps errors before or at the start visible (missing folder, failed clone, a
  CLI that cannot start). After the agent call a normal end (exit code 0, 130, `0xC000013A`, or
  the call stopped by Ctrl+C) closes the terminal with `[Environment]::Exit(0)`, after the Docker
  token cleanup; any other code stays open with „[traygent] <Agent> wurde mit Code <n> beendet.“
  The install terminal always stays open.
- Docker: `docker run -it --rm` with the workspace bind-mounted, state in **named volumes**
  (never bind-mounts for state — agent-tools ADR-0007), Claude reuses the starter's volume
  layout so an existing login keeps working (research file has the recipe).
- Detection: local — find the CLI on PATH + known install locations, run `<cli> --version` with
  a timeout, capped output, closed stdin, no console window (`CREATE_NO_WINDOW`); docker —
  `docker version --format {{.Server.Version}}` with timeout (CLI missing → `missing`, daemon
  down → `stopped`). Detection runs at startup and on `refresh_detection`, off the UI thread.
- Persistence: settings + recents (+ the GitHub account's login, id and source, never its token)
  in one versioned JSON file in the app data dir
  (`state.json`, `{"version":1,...}`, atomic write via temp file + rename, a newer version is
  refused, a corrupt file is backed up and replaced by defaults). Theme stays in localStorage.
