# ADR-0009: Agent Profiles With Linked Configuration Directories

- **Status:** Accepted
- **Date:** 2026-11-13
- **Decider(s):** both
- **Tags:** agents, profiles, accounts, sync

## Context

The owner uses several Claude subscriptions on one machine and until now kept them apart with a script (`Claude-Manager`): one command per subscription, a login per subscription, and a copy step that brings the second one's configuration in line with the first (#33). He wants Traygent to own that: profiles per agent, chosen per start, without signing in again — and whatever profile is chosen, skills, agents, commands, plugins, settings, MCP servers and history are the same.

Claude Code keeps its sign-in in `.credentials.json` inside its configuration directory on Windows, and its documentation names one configuration directory per account (`CLAUDE_CONFIG_DIR`) as the way to stay signed in to several accounts. A second directory starts empty, though: everything the user configured lives in the first one. Copying it, as the script did, is a snapshot — a skill added or a session held in one profile is missing in the other until the next copy, and two copies of the same history drift apart.

## Decision

We will give every profile its own configuration directory and select it per start through the CLI's own variable. The profile „Standard“ is the CLI's own home (`%USERPROFILE%\.claude`) and always exists; every other profile lives in `%USERPROFILE%\.claude-<id>` and is started with `CLAUDE_CONFIG_DIR` set to it.

Everything except the sign-in is shared instead of copied wherever the file system allows it: the directories that hold configuration and history (`skills`, `agents`, `commands`, `rules`, `output-styles`, `workflows`, `agent-memory`, `themes`, `plugins`, `projects`, `file-history`, `plans`, `tasks`, `paste-cache`) are directory junctions into the Standard home, so both profiles read and write the same files at every moment. Loose files cannot be junctions, so Traygent syncs them when the app starts, before every local start, after a change of the profile list and on demand: `settings.json`, `CLAUDE.md` and `keybindings.json` (newest wins), `history.jsonl` (line union) and the keys `mcpServers` and `projects` of `.claude.json` (from the newer file). `.credentials.json` and every other key of `.claude.json` — the account identity among them — are never read into another profile. On a profile's first sync the Standard home wins; data the sync displaces is merged into the Standard home where it is missing there and moved to `<profile>\.traygent-backup\<time>\`, never deleted.

The model (`settings.profiles`, `settings.agentProfile`, `LaunchRequest.profile`) is per agent and the sync recipe is data, so Codex (`CODEX_HOME`) and OpenCode can follow; the first cut covers Claude Code in the local environment.

## Consequences

### Positive

- The vendor's documented mechanism: no patched credentials, no token handling in Traygent, every claude.ai feature of each account keeps working (Remote Control, connectors).
- Shared directories cannot drift — there is one copy. A session started in one profile can be resumed in the other.
- A profile named like an existing directory (`team` → `.claude-team`) adopts it together with its login.
- Nothing is deleted; every displaced file has a dated backup.

### Negative / Trade-offs

- Loose files are only as fresh as the last sync: a change made in a running session reaches the other profile with the next start through Traygent, and a session started outside Traygent syncs nothing.
- Newest-wins on whole files loses one side when both profiles changed the same file between two syncs.
- `.claude.json` is rewritten by running sessions; Traygent's merge of its two shared keys can race with such a write. It only writes when the keys differ.
- Plugins synced from the claude.ai account and organization-managed settings follow the account; with shared `plugins` the last session's account decides.
- Junctions need NTFS and both directories on a local volume; a profile directory on a network share cannot be linked.
- Removing a profile leaves its directory, including the login, on disk.

### Neutral

- The Standard home is the sync source by definition; there is no setting for it.
- Docker sessions keep their one volume ([ADR-0004](0004-docker-volumes-shared-with-the-batch-starter.md)) and ignore the profile.

## Alternatives Considered

- **One configuration directory and a long-lived token per profile** (`claude setup-token` → `CLAUDE_CODE_OAUTH_TOKEN` from the OS vault) — rejected as the default: nothing to sync, but that token only makes model requests (no Remote Control, no claude.ai connectors), expires after a year, and a secret would have to travel into every session's environment.
- **Copy everything, as the script did** — rejected: a snapshot per run, duplicated history, and no answer to changes made in the second profile.
- **Swap `.credentials.json` in the one home before each start** — rejected: two sessions of different profiles could not run side by side, and a refreshed token would be written into the wrong slot.
- **File symlinks for the loose files too** — rejected: they need Developer Mode or elevation on Windows, and the CLI replaces these files on write, which would break the link.
- **A junction crate** — not taken: a new runtime dependency for one call that PowerShell already offers (`New-Item -ItemType Junction`), needed only when a link is missing.

## References

- `src-tauri/src/profiles.rs` and its submodules, `src/lib/ProfileSettings.svelte`
- `docs/SPEC.md → Profiles`, `docs/AGENT_RECIPES.md`
- Claude Code documentation: _Authentication → Log in with multiple accounts_, _Credential management_
