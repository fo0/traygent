# ADR-0008: Background Updates of the Agent CLIs

- **Status:** Accepted
- **Date:** 2026-11-10
- **Decider(s):** both
- **Tags:** agents, updates, processes

## Context

The owner wants the connected agent CLIs (Claude Code, OpenCode, Codex) kept up to date by Traygent: at app start and every four hours, but only while the CLI is not in use (#16). The three CLIs differ in how they are installed (native installer, WinGet, npm) and in whether they update themselves; each one ships its own update command that knows its install method. Replacing a running CLI can break the session using it, and Traygent does not own the sessions — a terminal it opened lives on its own, and a CLI can be started without Traygent at all.

## Decision

We will run each locally installed CLI's own update command (`claude update`, `opencode upgrade`, `codex update`) on its resolved path, hidden, with a 10-minute timeout, when `settings.agentUpdates` is on (default off): about a minute after the app started and then every 4 hours, plus on an explicit „Jetzt aktualisieren“. Traygent neither downloads nor compares versions itself.

A CLI counts as in use while a process of it runs — its own executable, or a `node` / `bun` process whose command line names its package — read right before each agent's turn from `Get-CimInstance Win32_Process` through Windows PowerShell. The check fails closed: a process list that cannot be read, times out or arrives incomplete skips every agent. A local start of an agent that is being updated is refused with a message. The result per agent (`running`, `updated`, `current`, `inUse`, `failed`) lives in memory and reaches the panel as `Snapshot.agentUpdates`.

## Consequences

### Positive

- One code path for every install method; a vendor changing its updater needs no change here.
- Sessions started outside Traygent protect the CLI as well, because the check reads processes, not Traygent's own bookkeeping.
- Nothing runs without the user switching it on.

### Negative / Trade-offs

- A session that starts between the check and the update can still meet a CLI that is being replaced; only starts through Traygent are refused.
- A CLI that is always open never updates — the status says „in Benutzung“.
- The update command's output is not shown; a failure is one word, and the user falls back to the command in Settings → Agenten.
- A PowerShell start every four hours per run; machines that block PowerShell get „in Benutzung“ forever (fail closed).

### Neutral

- Docker sessions are untouched: the CLI in a container volume keeps its own update path ([ADR-0004](0004-docker-volumes-shared-with-the-batch-starter.md)).
- The last-run time is not persisted; every app start is a run.

## Alternatives Considered

- **Compare against the vendor's latest version first** (npm registry, release feeds) — rejected: three feeds to track, and the update commands already do it.
- **Track only sessions Traygent started** — rejected: misses sessions opened by hand, and Traygent does not learn when a terminal closes (#17 would add that).
- **A process-list crate (`sysinfo`) or Toolhelp32** — rejected: a new runtime dependency, respectively no command lines, so npm installs running as `node.exe` would be invisible.
- **On by default** — rejected: installing software unasked is the user's decision.

## References

- `src-tauri/src/agent_updates.rs`, `src-tauri/src/desktop_agent_updates.rs`, `src-tauri/src/service/agent_updates.rs`, `src/lib/AgentUpdates.svelte`
- `docs/SPEC.md → Agent updates`, `docs/AGENT_RECIPES.md` (update commands)
