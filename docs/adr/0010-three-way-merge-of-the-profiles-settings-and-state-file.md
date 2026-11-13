# ADR-0010: Three-Way Merge of the Profiles' Settings and State File

- **Status:** Accepted
- **Date:** 2026-11-13
- **Decider(s):** both
- **Tags:** agents, profiles, accounts, sync
- **Supersedes:** ADR-0009 in part — the sync of `settings.json` and `.claude.json` and the list of shared entries

## Context

[ADR-0009](0009-agent-profiles-with-linked-config-directories.md) syncs `settings.json` as a whole file (newest wins) and merges only the keys `mcpServers` and `projects` of `.claude.json`, telling a removed entry from an added one by the entry names kept in the marker. The owner's `Claude-Manager.ps1`, which the profiles replace, does more (#35): it merges `settings.json` key by key against a stored base and keeps the keys that carry a sign-in or route requests (`forceLoginOrgUUID`, `apiKeyHelper`, the auth variables of `env`, …) in their profile. Copied as a whole, a profile pinned to one organization pins every other profile to it.

The names-only base could not tell which side changed an entry both still hold, so an edit to one project field and another edit to a different field of the same project left one of them lost. The script also shares more directories (`hooks`, `todos`, `session-env`), and the owner keeps tool state of his own (`.caveman-sessions`, `orca-state.json`) that a fixed recipe cannot know.

## Decision

We will merge `settings.json` and the shared subset of `.claude.json` three ways against a value base per profile, `<profile dir>\.traygent-base\<file>`: the shared part both sides held after the rule's last successful sync. Who differs from the base changed something; a removal travels; objects merge key by key at every depth, `projects` entries field by field; string lists under `permissions.*` and `autoMode.*` merge as sets; where both sides changed one value differently the newer file wins, and without a base of its own the Standard home wins and nothing is removed. The keys and `env` variables of the sign-in and of request routing (the recipe's `SettingsRule`) and a project's runtime fields (names starting with `last`, `hasUnseenTeamArtifacts`) never leave their profile and never reach a base. `.claude.json` additionally shares `showSpinnerTree` and `claudeInChromeDefaultEnabled` as one value each. A pair without a base learns what the Standard home removed from another profile's base, so a new profile never brings a removed entry back; a side that suddenly holds none of two or more known keys counts as a reset and is united, not mirrored. The marker's name-only `base` is read once on upgrade and no longer recorded.

The shared directories gain `hooks`, `todos` and `session-env`, and `settings.profileShare` lets the user add directories (linked like the recipe's) and files (whole-file, newest wins, like `CLAUDE.md`) per agent, by plain names that are validated on save, on load and again by the sync. `CLAUDE.md` and `keybindings.json` keep the whole-file rule.

## Consequences

### Positive

- One profile's organization pin, API key helper or cloud routing never reaches another profile; everything else in `settings.json` still does.
- Edits both profiles made to different keys, list items or project fields between two syncs both survive.
- A removed setting, MCP server, project or whole-value key propagates; a new profile does not resurrect it.
- The user can share tool state Traygent does not know without waiting for a recipe change.

### Negative / Trade-offs

- A conflict (both sides changed one value) is decided silently by file time; the loser survives only in the backup and the panel does not say so.
- The base holds values, MCP server commands and their `env` included, in a second file per profile; it stays in that profile's own directory and never crosses homes.
- Routing variables outside the recipe's lists (`CLOUD_ML_REGION`, `VERTEX_REGION_*`, `CLAUDE_CODE_SKIP_*_AUTH`) are shared.
- Extra files are copied whole; a JSON extra file gets no merge.
- Installs that already ran the whole-file `settings.json` copy of ADR-0009 may hold another profile's sign-in keys; the merge keeps them as that profile's own and does not clean them up.
- Removals still need a base: a profile added after a removal was synced everywhere has nothing to learn it from and brings the entry back.

### Neutral

- The merge runs in Rust over std only (`profiles/merge.rs`, `node.rs`, `pair.rs`); no new dependency.
- Numbers keep the token the file wrote; two spellings of one number compare equal.

## Alternatives Considered

- **Keep `settings.json` whole-file and strip the sign-in keys on copy** — rejected: the copy still overwrites every other edit of the losing side, and a stripped key would be deleted on the side that owned it.
- **Keep the names-only base for `.claude.json`** — rejected: it cannot tell which side changed an entry both hold, and gives no base at all for whole-value keys.
- **A base in the marker, holding values** — rejected: the marker is read to decide whose a directory is and stays small and free of commands and tokens; a separate file per merged file keeps both apart.
- **A JSON merge for user-defined extra files** — not taken: Traygent knows nothing about their structure or which of their keys are per profile.

## References

- #35, PR #36; `src-tauri/src/profiles/{settings,state,merge,pair,node,json,marker,share,foreign}.rs`, `src-tauri/src/settings/share.rs`, `src/lib/ProfileShareFields.svelte`
- `docs/SPEC.md → Profiles`, [ADR-0009](0009-agent-profiles-with-linked-config-directories.md)
