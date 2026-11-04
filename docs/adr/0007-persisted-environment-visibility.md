# ADR-0007: Persisted Environment Visibility

- **Status:** Accepted
- **Date:** 2026-11-04
- **Decider(s):** both
- **Tags:** settings, storage, ui

## Context

The composer always offered Lokal, Docker and WSL, WSL disabled as „bald“. The owner wants to switch environments on and off in the settings so the ones nobody uses stop getting in the way (#17). Recents, the default environment, the Docker settings and the state file (ADR-0002) all name an environment, so hiding one has to say what happens to each of them. WSL cannot start a session yet, so a list that keeps only WSL would leave the composer with nothing that starts.

## Decision

We will persist `settings.environments: EnvId[]`, the environments the composer shows, default all three. It is a display preference, not a ban: recents of a hidden environment keep it and ▶ restarts them unchanged, „In Composer übernehmen“ falls back to the first shown environment, and detection still probes Docker. WSL does not count toward „one must stay on“: the list must keep Lokal or Docker, and the panel locks the last of them (`aria-disabled`, focusable, with its reason). The default environment must be a shown one; hiding it moves the staged default to the first shown environment. Rust normalizes the list to each id once in `local` · `docker` · `wsl` order and refuses an empty one, a WSL-only one and one without the default, with German errors. In `state.json`, a missing field means all three, an unknown id is dropped on its own, and an empty or WSL-only list loads as all three; the `version` stays 1. While the staged list hides Docker, its settings section and the Agenten tab's Docker readiness are hidden too, and unticking Docker resets each invalid Docker field (the default ports count as one) to its saved value while valid edits stay staged.

## Consequences

### Positive

- The composer, the default-environment row and the footer show only what the user uses; with one environment, the composer's row disappears.
- A hidden field can never block „Speichern“, and nothing half-typed is saved.
- Old state files keep today's behaviour with no migration.

### Negative / Trade-offs

- An older build ignores the field and drops it on its next save, so a downgrade shows all three again.
- Unticking Docker discards an invalid staged Docker value without asking. The default ports are one field: a list with a conflict (e.g. a host port used twice) resets as a whole, valid port edits in it included — which of two conflicting entries is the valid one is not knowable.

### Neutral

- Validation of the Docker fields is unchanged; only what is in view and what unticking resets changed.

## Alternatives Considered

- **A flag per environment** (`{ local: true, … }`) — rejected: the list mirrors `AgentCatalogEntry.environments` and needs no new struct on either side.
- **A ban on hidden environments** (refuse their recents) — rejected: hiding is about the composer, and refusing a one-click restart would surprise.
- **Keep an invalid Docker section visible after unticking** — rejected: the section unmounted mid-typing once the value turned valid, lost focus and saved a partial value.
- **Count WSL toward the last environment** — rejected: a WSL-only composer could not start anything.

## References

- `src-tauri/src/settings.rs`, `src-tauri/src/store.rs`, `src/lib/environments.ts`, `src/lib/settingsDraft.svelte.ts`
- `docs/SPEC.md → Settings dialog`, [ADR-0002](0002-versioned-json-state-file.md)
