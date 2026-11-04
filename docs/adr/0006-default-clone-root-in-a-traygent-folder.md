# ADR-0006: Default Clone Root in a Traygent Folder

- **Status:** Accepted
- **Date:** 2026-11-04
- **Decider(s):** owner
- **Tags:** launch, github
- **Supersedes:** ADR-0005 (only the default clone root and the restart folder: ▶ on a repository recent sends its folder with the repository)

## Context

[ADR-0005](0005-start-from-a-github-repository.md) starts a repository in `<clone root>\<owner>\<name>`, the clone root taken from Settings → GitHub („Klon-Ordner“), and set the default clone root to `%USERPROFILE%\source\repos`, the folder Visual Studio also uses. The owner wants the default to be a named folder in the user profile instead: `%USERPROFILE%\Traygent` (#17). Release v2026.1104.711 already shipped repository starts with the old default (v2026.1103.2346 predates them), so installed copies can hold „Zuletzt“ entries whose clones live under `%USERPROFILE%\source\repos`. Until now, ▶ on such an entry sent only the repository, so with a new default it would clone a second copy into the new root (BACKLOG #40).

## Decision

We will use `%USERPROFILE%\Traygent` as the default clone root: `repo.rs → default_clone_root` resolves it from `USERPROFILE` and normalizes it like any folder, and the panel receives it as `github.defaultCloneRoot`, the „Klon-Ordner“ placeholder. A `cloneRoot` the user set is used unchanged, and `''` still means the default. Together with it, ▶ on a „Zuletzt“ entry started from a repository sends the entry's folder with the repository; Rust starts in that folder, without a clone, while it exists, and derives the folder from the current clone root (reuse or clone, as in ADR-0005) only when it is gone (#40). Everything else in ADR-0005 stays in force.

## Consequences

### Positive

- Repositories Traygent clones sit in one folder named after the app, apart from other tools' `source\repos`.

### Negative / Trade-offs

- A new start from the folder field (typed, picked or adopted from a „Zuletzt“ entry as `owner/name`) of a repository already cloned under `%USERPROFILE%\source\repos` clones a second copy, unless the user sets the „Klon-Ordner“ to that folder.
- When a „Zuletzt“ entry's folder is gone, ▶ clones into the current root and records a new entry; the old one stays until the user removes it.

### Neutral

- No migration: although v2026.1104.711 shipped the old default, the reuse rule (#40) keeps every existing clone of a „Zuletzt“ entry in use, and the state file keeps `cloneRoot` `''` (now the new default) unless the user set a folder, which stays unchanged.

## Alternatives Considered

- **Keep `%USERPROFILE%\source\repos`** — not chosen: the owner request.
- **A folder under `%LOCALAPPDATA%`** — not chosen: working copies the user opens and edits belong in a visible folder, not in app data.

## References

- #17, BACKLOG #40, [ADR-0005](0005-start-from-a-github-repository.md), `agent_docs/ux_rules.md` (rule 11), `src-tauri/src/service/launch_prep.rs → repo_target`
