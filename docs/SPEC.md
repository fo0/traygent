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
- **Profile** (`AgentProfile`, per agent, local and Docker starts): a sign-in of one CLI in a config
  directory of its own; everything else stays identical to the CLI's own home, the profile
  „Standard“ (id `default`) every agent with `catalog.profiles` has. Claude Code only so far;
  see _Profiles_ below.

## Dashboard (single panel, compact — `agent_docs/ux_rules.md` is binding)

1. AppBar: brand · refresh-detection icon · settings gear.
2. **Neue Session** composer (one card, no prose), directly under the AppBar like RepoWatch's
   search field — no visible section heading; the section is named for assistive tech only and
   the checked agent is the page's focus landing:
   - The selected agent shows its name and environment status on the left. „Stand hh:mm“ on the
     right is the oldest successful quota read among its shown profiles. Hovering over it shows
     the refresh interval. Below, per shown profile (#94: the start profile plus those chosen
     with „Im Tile zeigen“, at most 3, in list order; each one read with its own profile) one
     label line and that profile's bars stacked directly under it (3px tracks, no caption
     between them). The line reads `<Profil>: Woche 64 % · Fable 81 %` — the enabled windows in
     their configured order, each word and figure in the metric's colour (`--quota-*` tokens),
     the figure the **used** share — with the weekly reset countdown right-aligned („3 Tage“,
     „5 Std.“, nothing when unknown). The start profile's name is bold; an agent without extra
     profiles shows the line without a name. From 70 % used the figure turns `--wait`, from
     90 % `--fail` and bold, and the bar ends in a short cap of that colour. No data, a
     signed-out profile or a failed read: `–` and a dashed track, the detail in the tooltip.
     Claude Code's figures come from its own CLI, `claude -p /usage --output-format json
--no-session-persistence --setting-sources user` run hidden on the host with the profile's
     sign-in (also for a Docker session; never in a profile directory that is missing or not
     Claude Code's; [ADR-0015](adr/0015-read-the-claude-code-quota-through-the-cli.md)); Codex
     and OpenCode Go are read from the provider (ADR-0014).
     A read the provider limits (HTTP 429, #100; `get_agent_quota` answers no windows, the
     detail and `retryAfterSecs`, its `Retry-After` in seconds, at most a day, `0` without one)
     keeps the last figures with their „Stand“ until their own reset; the tooltip names the
     next attempt („Zu viele Abfragen. Nächster Versuch um hh:mm.“, „am TT.MM. um hh:mm“ when
     it is not today). Reads of one provider run one at a time, at least 5 s apart (plus up
     to 2 s jitter); a limited account waits 5, 10,
     20, 40, then 60 minutes or its `Retry-After`, whichever is longer, also on a manual refresh,
     while the automatic read still waits for its regular interval.
     The line never wraps: the profile name ellipsizes first. The tooltip and accessible
     description carry per profile and metric the used and free share, the reset date, the
     detail and the refresh hint. The other agent buttons keep their mark, status dot and one
     bottom bar of weekly consumption in state colours (their saved default profile).
     In Settings → Agenten, users can enable and order additional windows for the active button.
     The week stays enabled. Missing or expired values never count as an unused allowance.
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
     `profiles`, environment Lokal or Docker and at least one extra profile. A 28 px mark only — the
     person icon for „Standard“, else the profile's initial on a round tile; the label is its
     accessible name and tooltip (`Profil: <label> · <account>`). The chosen profile not signed
     in → warning colour and „Nicht angemeldet“ (tooltip and list). Starts at the agent's saved
     „Standard-Profil“, back to it after every start and agent change; a removed profile falls
     back to it. Sent only when it applies and is not `default`.
   - Primary button „Starten“ bottom right; Enter starts. Missing CLI → button becomes
     „Installieren …“ (opens Settings → Agenten for that agent) — never a dead start. While
     blocked, the reason is the button's tooltip; a press shows it as an error line below the
     button until nothing blocks.
   - Options start from the agent's saved defaults; changing them on the dashboard affects the
     next start only (not the defaults).
3. **Workspaces** (recent sessions — where RepoWatch lists repositories): one single-line row per
   (folder, agent, env), newest first, max 20 kept, in one bordered list card with dividers
   (RepoWatch's list card). Row: agent mark on a tile — a profile other than „Standard“ sits on
   its corner as its initial on a small round accent tile („Profil <label>“, also in the row's
   tooltip), so it takes no width from the folder name — folder name (primary) + parent path
   (secondary, ellipsized), then fixed columns at the right edge so every icon lines up from
   row to row (`RecentMeta.svelte`; an absent part leaves an empty, `aria-hidden` slot): a `</>`
   mark („Extra-Argumente“) when a restart passes extra arguments and the ports chip (together
   the one variable width, first), GitHub mark, env icon (Lokal/Docker), the options as
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
   arguments, (Docker) ports and (an agent with at least one extra profile or an entry
   that still stores one, #37) „Profil“ — „Standard“ and the agent's profiles; a stored profile
   removed since shows as „Standard“, and saving stores that. „Speichern“ stores them on the entry without a start
   (`update_recent`, validated like a start; the dialog also applies the npm-shim argument rule) — place, start time and count stay, the next ▶
   sends them. The profile follows the start rule (_Profiles_): absent, blank or `default`
   store none; local and Docker entries retain a chosen profile. An id the agent does not list is refused („Profil nicht
   gefunden.“). The agent, environment and folder define the entry and are not editable.
   A row's profile: ▶ restarts with it. One removed since never falls back to another (#61):
   the tile's mark turns into a warning „?“ named „Profil entfernt“, ▶ and its Ctrl+digit are
   blocked („Profil entfernt · in „Bearbeiten …“ ein anderes wählen“), and a start that slips
   through is refused natively („Profil nicht gefunden.“); „In Composer übernehmen“ takes it
   over, or the saved „Standard-Profil“ when it is gone. A start of the same folder with another profile updates the row's profile.
   „Modell & Denken“ (#43, a `Segmented` at the top of „Bearbeiten …“, explanation in the
   tooltips): „Traygent“ (default) — every ▶ sends the stored model and thinking level as
   today; „Agent“ — ▶ sends `keepAgentModel: true` and the launch builder passes no model or
   effort to any agent (Claude `--model` / `--effort`, Codex `-m` /
   `-c model_reasoning_effort`, OpenCode's `model` key in `OPENCODE_CONFIG_CONTENT`), so the
   agent continues with what it last used (e.g. after `/model` in Claude Code); permission,
   session, extra arguments, profile and ports are unchanged. While „Agent“ is chosen the
   model and thinking fields are off with their values kept, and the row shows one mark
   („Modell & Denken vom Agent“, also in ▶'s tooltip) in the model column instead of the model
   badge and the signal bars. `RecentSession.keepAgentModel` changes only through
   `update_recent`; a start never changes it, and a flagged start keeps the stored model and
   thinking level (it never overwrites them). A composer start always sends the composer's
   model and thinking level (no `keepAgentModel`); on the same workspace it keeps the mode and
   stores those options as today.
4. Footer: Docker state („Docker 27.5 bereit“ / „Docker nicht erreichbar“, only while Docker is
   shown) · „Stand hh:mm“.

## Settings dialog (staged: „Speichern“ + „Abbrechen“; tabs)

- **Agenten**: per agent a section, opening with „Kontingent“: the staged switch „Kontingent
  abfragen“ (`settings.quotaReads`, default off for Claude Code and Codex, on for OpenCode;
  tooltip for Claude Code „Startet die Claude-Code-CLI (/usage) mit der Anmeldung dieses
  Profils“, for Codex „Nutzt die CLI-Anmeldung gegen eine nicht offiziell dokumentierte
  Schnittstelle des Anbieters“, for OpenCode „Nutzt den eigenen OpenCode-Go-Schlüssel aus der
  OpenCode-Anmeldung“; [ADR-0014](adr/0014-opt-in-subscription-quota-reads.md),
  [ADR-0015](adr/0015-read-the-claude-code-quota-through-the-cli.md)), then the bar choices
  (`settings.quotaBars`, the week always on, tooltip „Bei allen CLIs immer sichtbar“; disabled
  while the read is off) and a „Vorschau“ tile. With the read off the panel never calls
  `get_agent_quota` for that agent, drops its cached reading, and its composer button shows no
  figure and no bar, only the description „Kontingentabfrage ausgeschaltet“. Then local status (version or „Nicht installiert“), Docker
  readiness (only while Docker is shown), best-practice Windows install command (copy button + „Installieren“ = opens a
  terminal running it, then re-detects on demand), update command; defaults: model, effort,
  permission, extra CLI args (free text, validated). For an agent with `profiles` a „Profile“
  block (info tip „Eigene Anmeldung je Profil, alles andere wird abgeglichen. Weitere Ordner und
  Dateien werden wie die eingebauten abgeglichen.“): „Jetzt
  synchronisieren“ beside the heading (immediate, `sync_profiles`, tooltip „Automatisch alle 15
  Minuten“), „Standard-Profil“ (staged), one single-line row per profile — „Standard“ first —
  with a chevron toggle and its name, a sign-in dot (#43: green `--ok` signed in, red `--fail`
  signed out; `role="img"` named „Angemeldet“ while signed in, hidden from assistive tech while
  signed out, as the account text says it already; none until the snapshot
  lists the profile; a forced-colours ring when signed out), the account or „Nicht angemeldet“ — when another signed-in
  profile of the agent has the same account (ignoring case) but another organization, the
  organization instead of the account (#41); tooltip: account · organization —, a warning icon
  after a failed sync, the staged icon switch „Im Tile zeigen“ (#94, `settings.quotaProfiles`:
  only while the agent's quota read is on and a second profile exists; `aria-pressed`, tooltip
  „Im Tile zeigen · jedes gezeigte Profil ist eine weitere Kontingentabfrage“; the start profile
  always on and locked, a fourth disabled with „Höchstens 3 Profile“; a removed profile leaves
  the list; read through `quotaTypes.ts → shownQuotaProfiles`), „Anmelden“ only while the profile is signed out (immediate,
  `profile_login`, tooltip „Im Browser anmelden“) and a remove button (staged, undone by
  „Abbrechen“; none for „Standard“). While a hidden sign-in runs the same button turns into a
  spinning icon (accessible name „<Name>: Anmeldung abbrechen“, `aria-busy`, tooltip „Anmeldung
  läuft · Abbrechen“) whose click sends `cancel_profile_login`; the second click of a
  multi-click and an auto-repeated Enter or Space never act (`rowStart.ts → repeatClick` /
  `repeatKey`), so they cannot cancel the sign-in their first press started. Cancel exists for
  the hidden sign-in only: at the log level „Debug“ the button just waits until the terminal opened —
  spinning, `aria-busy`, `aria-disabled`, name „<Name> anmelden“ and tooltip unchanged, no
  action. A second sign-in of a pending profile is ignored in both modes. Success —
  only when the answer reads the profile signed in — shows the answer's statuses and the 3 s
  note „Angemeldet.“, and the focus moves from the gone button to the row's details toggle; a
  cancel ends quietly (when `cancel_profile_login` found nothing to end, a later failure is
  still reported); a failure stays as the block's inline error „<Name>: <reason>“ (tooltip „Mit
  Debug erneut versuchen“ after a hidden sign-in) until that profile signs in again, „Jetzt
  synchronisieren“ or an added profile. The state lives in the panel
  (`profileActions.svelte.ts`), so closing the dialog keeps a running sign-in. The toggle (`aria-expanded`) opens
  the row's details in place, one row at a time: „Name“ (for an extra profile a field holding a
  staged rename — the id and folder never change — checked by the list rules — a problem shows in the open detail, else under „Neues Profil“ as „<Name>: <problem>“, and in the toggle's description; read-only for
  „Standard“), „Konto“, „Organisation“ (when reported; „Konto“ of a signed-in profile carries „Neu anmelden“ on the
  right, the same action), „Abo“ (the `plan` capitalised, `max` → „Max“, signed in only — also
  appended to the row's account tooltip; #43), „Ordner“ (the folder, mono, with a copy
  button), „Abgeglichen <time>“ and „Abgleich“ with the sync error when present. Then „Neues
  Profil“ + „+“ / Enter, whose tooltip shows the folder the name gives: adding is an explicit
  immediate action (#37) — it sends `save_settings` with the saved settings plus the new profile,
  never the other staged edits, takes the answer as the new baseline and keeps every staged edit
  staged, so the row signs in at once; meanwhile the field is busy, „+“ turns into a spinning
  „Wird hinzugefügt …“, and „Speichern“, „Abbrechen“, Escape and the close button wait (the
  dialog cannot close on a half-applied add). A name whose id the saved list still holds (a staged removal) brings that
  profile back without a save, and the saved default with it when it was that one. Inline errors: „Name fehlt.“, „Höchstens 24 Zeichen.“, „Keine
  Steuerzeichen.“, „„<Name>“ ist reserviert.“, „Mindestens ein Buchstabe oder eine Ziffer.“,
  „Profil gibt es schon.“, „Höchstens 8 Profile.“ (with „ Erst speichern.“ appended when only
  a staged removal or rename frees the name or the place), and a native refusal verbatim; the
  name stays typed. With at least one extra profile staged, „Weitere Ordner“ and „Weitere
  Dateien“ (`profileShare`, staged; names split at commas and whitespace, repeats once, tooltip
  „Durch Komma oder Leerzeichen getrennt“, placeholders `.caveman-sessions` / `orca-state.json`)
  with the native errors of _Profiles → Extra entries_ inline, the first problem per field, and
  below them „Bereits abgeglichen“, collapsed by default: the built-in folders and files, mono
  (`profileShare.ts → BUILT_IN_SHARE`, as `share.rs → name_lists` splits them). Removing the last
  extra profile hides them; a list that is then invalid goes back to its saved value.
- **Umgebung**: „Umgebungen“ — one checkbox per environment the composer shows (Lokal / Docker /
  WSL, all on by default; the last of Lokal and Docker cannot be turned off and says why in its
  tooltip), default environment (one of the shown ones, WSL disabled; it moves to the first shown
  one when its own is hidden, the row hides with a single one), default agent, terminal
  (Windows Terminal / Konsole; with Windows Terminal, `sharedWindow` (default off) opens every
  start as a tab of one window named `traygent`), Docker: image per agent (defaults from research), forward the connected Traygent account’s token to Git and gh in the container (off by default), „Docker-Socket durchreichen“ (`docker.socket`, off by default, tooltip
  only; _Docker socket_). Rust stores the list each once in `local` · `docker` · `wsl` order and
  refuses one without Lokal or Docker („Mindestens eine Umgebung muss aktiv sein.“, „WSL ist noch
  nicht verfügbar.“) or without the default („Die Standard-Umgebung ist deaktiviert.“); a
  `state.json` without the field shows all three, an unknown id is dropped. While the staged list
  hides Docker, the Docker section (images, default ports, gh token, socket) and the Agenten tab's Docker
  readiness are hidden too, their values kept; unticking Docker resets each invalid Docker field to
  its saved value, valid edits stay staged; the default ports count as one field, so a list with a
  conflict resets as a whole
  ([ADR-0007](adr/0007-persisted-environment-visibility.md)).
- **GitHub**: the one account (`login · Token` / `login · gh CLI`, „Trennen“ with inline
  confirm, „Aktualisieren“ for the listing) or, without one, two ways to connect: „Lokale
  Anmeldung prüfen“ (lists gh sign-ins, one consent line, „Übernehmen“) and a masked token field
  with „Verbinden“. Account actions apply at once; the clone root („Klon-Ordner“, placeholder
  the default) is staged like every setting.
- **App**: theme (System/Hell/Dunkel), quota refresh interval (1–60 minutes, default 15), Mit Windows starten, „Protokoll“
  (#109): the staged radio group „Info“ / „Debug“ (`settings.logLevel`, `"info" | "debug"`,
  default `info`; tooltip on „Debug“ „Ausführliches Protokoll; Profil-Anmeldung im sichtbaren
  Terminal“) and, natively only, the immediate button „Ordner öffnen“ (`open_log_folder`; a
  failure stays inline in the section). „Debug“ adds every quota read to the local log
  (`docs/ARCHITECTURE.md → Local log`) and opens a profile sign-in in the visible terminal
  instead of running it hidden (#41). Saved with „Speichern“, applied at once and at every start;
  a file from before #109 with `debug: true` loads as `debug`, an unknown value as `info`.
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
`LaunchRequest.keepAgentModel` and `RecentSession.keepAgentModel` (optional, absent = false,
#43 — _Dashboard_ → Workspaces), `DockerSettings.socket` (default false) and
`LaunchRequest.dockerSocket` / `RecentSession.dockerSocket` (optional, absent = false, Docker
only — _Docker socket_),
`Snapshot.profiles` (`ProfileStatus[]`: `agent`, `id`, `label`, `dir`, `signedIn`, optional
`account`, `organization`, `plan`, `syncedAt`, `syncError`) and `LaunchOutcome.warning` (optional,
German: the session started, the profile sync before it was incomplete).

Commands (Rust `#[tauri::command]`, German `Err` strings; panel calls only via `src/lib/ipc.ts`):

| command                            | args                                                                                             | result                                                                                      |
| ---------------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------- |
| `snapshot`                         | –                                                                                                | `Snapshot` (cached detection)                                                               |
| `get_agent_quota`                  | `{ agent, profile, env }`                                                                        | `AgentQuota` (reported windows with usage and reset time; read off → no windows, no detail) |
| `refresh_detection`                | –                                                                                                | `Snapshot` (re-probes CLIs + Docker)                                                        |
| `save_settings`                    | `{ settings }`                                                                                   | `Snapshot` (validated; invalid → Err)                                                       |
| `launch_session`                   | `{ request }`                                                                                    | `LaunchOutcome` (validates, opens terminal, records recent)                                 |
| `remove_recent`                    | `{ id }`                                                                                         | `Snapshot`                                                                                  |
| `restore_recent`                   | `{ recent }`                                                                                     | `Snapshot` (undo of remove)                                                                 |
| `update_recent`                    | `{ id, options, ports, profile: string \| null, keepAgentModel, dockerSocket }`                  | `Snapshot` (edit, no start)                                                                 |
| `pick_folder`                      | `{ initial?: string }`                                                                           | `string \| null` (native folder dialog)                                                     |
| `install_agent`                    | `{ agent }`                                                                                      | `void` (opens a terminal running the catalog install command)                               |
| `open_folder`                      | `{ path }`                                                                                       | `void` (Explorer)                                                                           |
| `update_agents`                    | –                                                                                                | `Snapshot` (starts an agent-CLI update run, events follow)                                  |
| `github_connect_token`             | `{ token }`                                                                                      | `Snapshot` (verifies `/user`, vaults the token, lists repos)                                |
| `github_local_accounts`            | –                                                                                                | `LocalAccounts` (gh sign-ins; reads no token)                                               |
| `github_import_local`              | `{ login }`                                                                                      | `Snapshot` (explicit consent: reads that gh token, as above)                                |
| `github_disconnect`                | –                                                                                                | `Snapshot` (deletes the vault copy; gh stays signed in)                                     |
| `github_refresh_repos`             | –                                                                                                | `Snapshot` (returns with `github.loading`, event follows)                                   |
| `sync_profiles`                    | –                                                                                                | `Snapshot` (syncs every profile now, statuses in `profiles`)                                |
| `profile_login`                    | `{ agent, profile }`                                                                             | `Snapshot` (hidden: after the sign-in; level „Debug“: once opened)                          |
| `cancel_profile_login`             | `{ agent, profile }`                                                                             | `boolean` (ends that profile's sign-in; false: none running)                                |
| `open_log_folder`                  | –                                                                                                | `void` (creates the log folder if missing, shows it in Explorer)                            |
| shell commands kept from RepoWatch | autostart_status, set_autostart, open_startup_settings, check_update, install_update, hide_panel | unchanged                                                                                   |

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
  holds one entry „GitHub verbinden …“ (Settings → GitHub) in their place. Up to 3 recent
  folders whose folder name (last segment) holds every word follow, newest first; a picked one
  becomes its full path. A repository typed or pasted needs no account.
- Start: `repo` set, `folder` `''` (composer) or the recent's folder (▶ on a „Workspaces“ row). A
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
  a same-named binary inside another program's bundle does not (the Claude and Codex desktop
  apps, the editor extensions of both — `agent_updates.rs → BUNDLED`; never the resolved CLI itself; one without a readable
  path still counts), and a `node` / `bun` process with an unreadable command line counts for
  every agent. `AgentUpdateStatus.by` names what held a skipped agent — the process's path,
  image name or `pkg` line, or a German reason — and is the row's tooltip (#78).
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
- **Start** (Lokal and Docker, ADR-0012):
  absent, blank or `default` = „Standard“; any other id must be listed („Profil nicht
  gefunden.“). No user profile → „Der Profilordner ist nicht verfügbar: kein Benutzerordner
  gefunden.“. Docker mounts the chosen home read-write. On local starts the script sets, after `Set-Location` and before the agent call,
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
  `sync_profiles`, a local or Docker start (that agent only) and a background sync every 15 minutes
  (`PROFILE_SYNC_INTERVAL`, fixed, no setting): the `traygent-profile-sync` thread checks once a
  minute and syncs every profile once 15 minutes have passed since the last sync of any kind
  finished (before the first one, since the app started), only while some agent has an extra
  profile and the user profile is known, also during an agent update (a sync never runs the CLI).
  It is silent apart from each profile's `syncedAt` / `syncError`, which reach an open panel as
  a `traygent:snapshot` event; a run that broke down also waits the full interval. A tick while
  another sync is in flight is skipped (that sync restarts the interval); a sync that starts
  between that check and the tick's own run is waited for, and the tick then runs the plan it took
  before. An agent without an extra profile is never touched. One sync runs at a time; every
  run's file system work happens without the service lock.
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
- **Never touched:** `.credentials.json` — never written, linked, copied or synced; it is read in
  place only for `signedIn` and `plan` (#43, the CLI's own rule, no CLI started): signed out when
  the file is missing or empty, has no `claudeAiOauth` object or a blank
  `claudeAiOauth.refreshToken`; signed in otherwise. A file that does not parse (the CLI may be
  mid-write), cannot be read or is over 1 MiB counts as signed in while it is not empty, so a
  race never flips the row. Only `claudeAiOauth.subscriptionType` leaves the read, as `plan`
  (signed in only, cleaned like `account`); no token is kept, logged or sent. The own
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
- **Sign-in (#41):** `profile_login { agent, profile }` („Standard“ included, only on a click)
  creates the profile directory and runs, in `%USERPROFILE%` (else `C:\`), the env line (for
  „Standard“ always the `Remove-Item`), the header room and `& '<cli>' 'auth' 'login'` (an npm
  `.cmd` shim the same way). Header room (#89): `--max-http-header-size=1048576` appended to
  `BUN_OPTIONS` and `NODE_OPTIONS` of that process — the browser sends every `localhost` cookie
  to the CLI's callback, and past the default 16 KiB the CLI answers 431 and never gets the code.
  At the log level „Debug“ the terminal stays open and keeps both variables for what is started in it. Refused
  before anything runs: „<Agent> wird gerade aktualisiert.“, „<Agent> ist
  nicht installiert.“, „Profil nicht gefunden.“, and „Der Profilordner gehört nicht zu Claude
  Code.“ for an existing foreign directory (the rule above, with the agent's extra directories;
  also when that cannot be checked) — a sign-in never writes credentials into it.
  - **Hidden** (`settings.logLevel` `info`, the default): the script, preceded by `$LASTEXITCODE=$null`
    and followed by `if ($null -eq $LASTEXITCODE) { exit 1 }; exit $LASTEXITCODE`, runs as
    `powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand <Base64>` without a
    window (`CREATE_NO_WINDOW`), stdin closed, stdout and stderr to the null device — the
    output can hold the sign-in URL or a code, so it never reaches Traygent's memory, a log or
    the panel. The CLI opens the browser itself. The command resolves when the process ended
    (checked before a cancel or the limit, so an end in the same tick counts as ended): exit
    code 0 → the agent's statuses are read anew (`SyncPlan::read`, without the service lock) and
    committed through `apply_profiles`; the `Snapshot` is returned only when the profile now
    reads signed in, else „Anmeldung fehlgeschlagen.“; any other code or a process that could
    not start → „Anmeldung fehlgeschlagen.“; after 5 minutes the process tree is ended
    (`detect/runner.rs → kill_tree`: absolute `taskkill /T /F`, bounded wait, then the process)
    → „Anmeldung abgelaufen.“; `cancel_profile_login { agent, profile }` ends it the same way →
    „Anmeldung abgebrochen.“ and answers whether one was registered — never for a visible
    sign-in, which has no ticket and no cancel. The sign-in is registered
    first thing in `profile_login`, before any work, so a cancel cannot miss it once the
    command runs; one per profile — a second one is refused with „Die Anmeldung läuft
    bereits.“; the registration ends with the worker, also when it panics. The app's exit
    (`RunEvent::Exit`: „Traygent beenden“, the updater's restart) cancels every running sign-in
    and waits at most 3 s for the trees (best effort). A prompt for a pasted code gets end of
    input (stdin closed) and ends the CLI or reaches the limit — it never hangs.
  - **Visible** (`settings.logLevel` `debug`): a terminal titled „<Agent> · <Label> anmelden“ runs the
    same lines and stays open like the install terminal (`-NoExit`), so every line of the CLI can
    be read; it is not registered and cannot be cancelled, and the command resolves with the
    current snapshot once it opened. A status watch
    (thread `traygent-profile-watch`) then reads the agent's statuses every 3 s (`SyncPlan::read`,
    never a sync, never under the service lock) until that profile's sign-in facts (signed in,
    account, organization) differ from those shown at the click, or 10 minutes passed; only a
    change is committed through `apply_profiles` and emitted as `traygent:snapshot`. One watch
    per profile: another click replaces it, a removed profile or a panicking read ends it.

## Linux runtimes in Docker

`settings.docker.manageNode` defaults to true, including for existing installations. The Docker section of **Umgebung** stages this switch with the other environment settings. All three agents use the workspace's first available `.nvmrc` or `.node-version`. Supported values are numeric majors, minors or releases with an optional `v` prefix, plus `lts` and `lts/*`. An unsupported value produces a notice. When there is one hook pin and no project version, it also supplies the session's Node. Otherwise the image supplies the default.

An exec-form Node hook in the selected Claude user settings can reference a mounted script by its Windows path. Traygent projects its argument list into Linux paths after profile sync and cloning. Each NVM hook can request its own exact release. Disabling mise selects the image runtime and retains path translation. The read-only settings projection belongs to this session. [ADR-0013](adr/0013-prepare-linux-node-for-docker-sessions.md)

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
- The custom field takes several entries at once, separated by blanks, commas or semicolons
  (`3000, 5173 8080:80`): all are added, or — when one is refused — none, and the field keeps
  the text with the first entry's reason.
- The service inside the container must listen on `0.0.0.0` (tooltip hint, no prose).

## Docker socket (Docker sessions, #46)

- Port of the Batch starter's „Docker Socket“ toggle: Settings → Umgebung → Docker
  „Docker-Socket durchreichen“ (`docker.socket`, app-wide, off by default, staged). The composer
  sends it with every Docker start (`LaunchRequest.dockerSocket`, no chip); the recent stores it
  and a row's ▶ sends the stored value (its tooltip then names „Docker-Socket durchgereicht“);
  „Bearbeiten …“ of a Docker row shows the same switch.
  Any other environment ignores it: launch preparation, the recent, an edit and a loaded
  `state.json` all normalize it to false, never an error.
- On: `-v /var/run/docker.sock:/var/run/docker.sock` (read-write — the agent drives the host's
  daemon, which is host-level access) next to the workspace mount, and a root block before the
  agent's init script: the static Docker CLI into `/home/node/.local/bin` when missing (latest
  from `download.docker.com/linux/static/stable/x86_64/`, fallback 27.5.1; a failure only warns;
  curl gives up after 10 s without a connection, after 30 s on the listing and after 30 s below
  1 KiB/s on the tarball, so a slow link still finishes),
  node joins the socket's group (a non-zero GID only), `chmod a+rw` when node still cannot use it,
  a German notice when no socket arrived, and `PATH` gains `/home/node/.local/bin` (`runuser -u`
  keeps it). `/home/node/.local` is a named volume for every agent, so the CLI persists.
- No probe before the start (the starter runs a throwaway container): an engine without the
  socket shows the in-container notice instead (`docs/VALIDATION.md`).

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
- The hidden sign-in (#41) is the one process without a terminal: `powershell.exe -NoLogo
-NoProfile -NonInteractive -EncodedCommand <Base64>`, the same argv rules (no `"`, no empty
  element), the same env line and literal call; see _Profiles → Sign-in_.
- Closing: `-NoExit` keeps errors before or at the start visible (missing folder, failed clone, a
  CLI that cannot start). After the agent call a normal end (exit code 0, 130, `0xC000013A`, or
  the call stopped by Ctrl+C) closes the terminal with `[Environment]::Exit(0)`, after the Docker
  token cleanup; any other code stays open with „[traygent] <Agent> wurde mit Code <n> beendet.“
  The install terminal always stays open.
- Docker bind-mounts the workspace and, for Claude Code and Codex, the host agent configuration read-write (ADR-0011). Linux tools and OpenCode state use named volumes. The Git config is mounted read-only when present and the Docker socket is optional (_Docker socket_).
- Detection: local — find the CLI on PATH + known install locations, run `<cli> --version` with
  a timeout, capped output, closed stdin, no console window (`CREATE_NO_WINDOW`); docker —
  `docker version --format {{.Server.Version}}` with timeout (CLI missing → `missing`, daemon
  down → `stopped`). Detection runs at startup and on `refresh_detection`, off the UI thread.
- Persistence: settings + recents (+ the GitHub account's login, id and source, never its token)
  in one versioned JSON file in the app data dir
  (`state.json`, `{"version":1,...}`, atomic write via temp file + rename, a newer version is
  refused, a corrupt file is backed up and replaced by defaults). Theme stays in localStorage.
