# ADR-0002: Versioned JSON State File Instead of SQLite

- **Status:** Accepted
- **Date:** 2026-11-03
- **Decider(s):** both
- **Tags:** storage, rust

## Context

The app shell comes from RepoWatch, which keeps its GitHub cache, favorites and quarantine in SQLite (`rusqlite`, `PRAGMA user_version`). Traygent persists far less: the settings (defaults per agent, environment, terminal, Docker images, the gh-token switch) and at most 20 recent sessions. Nothing is queried, nothing is cached from the network, and the whole state is read once at startup and written whole on every change.

A broken or foreign state file must never block a start or silently lose data: the user should be able to open, read and repair the file by hand.

## Decision

We will keep settings and recent sessions in one JSON file, `state.json` in the app data dir (`%APPDATA%\io.github.fo0.traygent\`), shaped `{"version":1,"settings":…,"recents":[…]}` (`src-tauri/src/store.rs`). Writes are atomic: a sibling `state.json.tmp-<pid>` is written, flushed and renamed over the file. A file without a valid `version` is renamed to `state.json.corrupt-<timestamp>` and replaced by defaults. A file with a higher `version`, or one that cannot be read, is used but never overwritten: saving returns a German error the panel shows. Unknown fields are ignored and missing or unreadable fields take their default. The theme stays in the panel's localStorage (`traygent.theme.v1`). No SQLite dependency is added.

## Consequences

### Positive

- One small, human-readable file; easy to back up, diff and repair.
- No native SQLite build in the Rust crate; the core stays `serde_json` only.
- A newer file is never clobbered by an older build; corrupt data is kept for inspection.

### Negative / Trade-offs

- Every change rewrites the whole file — fine at this size, not for a growing history.
- A schema change needs an explicit migration on `version`; tolerant field parsing hides typos in hand edits (they fall back to defaults).

### Neutral

- The settings and recents shapes follow the IPC contract (`src/lib/types.ts` ↔ `models.rs`), so the file mirrors what the panel sees.

## Alternatives Considered

- **SQLite as in RepoWatch** — rejected: no queries or cache to justify a database, and a native dependency for two lists.
- **Tauri store plugin** — rejected: one more plugin and capability for what `serde_json` plus a rename does, without control over the version and corrupt-file rules.
- **Settings in the registry** — rejected: not portable, not readable as one document, and recents do not fit.

## References

- `src-tauri/src/store.rs`, `src-tauri/src/service.rs`
- `docs/SPEC.md → Launch rules` (persistence)
