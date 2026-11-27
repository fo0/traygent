# ADR-0015: Read the Claude Code Quota Through the CLI

- **Status:** Accepted
- **Date:** 2026-11-27
- **Deciders:** Owner (request in chat, 2026-11-27) and implementation agent
- **Tags:** quota, process, privacy
- **Supersedes:** ADR-0014 for Claude Code only (the read itself and its tooltip); ADR-0014 keeps holding for Codex and OpenCode Go

## Context

ADR-0014 reads Claude Code's subscription quota from `api.anthropic.com/api/oauth/usage` with the access token from the profile's `.credentials.json`. That endpoint is not documented for third-party use, and the read takes a sign-in token out of the CLI's file. Claude Code itself answers the same question: `claude -p /usage --output-format json` prints one JSON object whose `result` is the `/usage` text — in about 1.5 s, with `num_turns: 0` and no tokens used (measured with CLI 2.1.332). The owner asked on 2026-11-27 (#110) to read the quota that way.

## Decision

We will read Claude Code's quota per profile by running the host's CLI, `<claude> -p /usage --output-format json --no-session-persistence --setting-sources user`, through the hidden PowerShell call of the profile sign-in (`launch/login.rs → hidden_usage_plan`), with the profile's `CLAUDE_CONFIG_DIR` set as a local start sets it, the sign-in override variables removed, Traygent's app data dir as the working directory (no read without one), stdin closed, stderr to the null device, stdout capped at 64 KiB and the process tree ended after 20 s or at the app's exit. A named profile's directory must exist and be Claude Code's, as for a sign-in. Only a printed object with `local_command: "usage"` is parsed; its `Current <label>: <N>% used · resets <when>` lines of `result` become the windows. Traygent never requests the OAuth usage endpoint and never opens `.credentials.json` for the quota.

## Consequences

### Positive

- No undocumented provider endpoint and no sign-in token leaves the CLI for Claude Code; the tooltip says what runs instead.
- The CLI refreshes its own sign-in; Traygent never sees one.
- The read compiles without the `updates` feature (no HTTP client).

### Negative / Trade-offs

- The `/usage` text is no contract. The parser is tolerant (unknown lines and labels are skipped, an unreadable reset time drops only the time) and pinned by `fixtures/claude-usage/`; a reworded line ends in „Die Ausgabe der Claude-Code-CLI ist nicht lesbar.“
- No year is printed; a date is the first at or after now − 1 day, in the machine's zone. A time the zone skips (a clock change) gives no reset. The zone the CLI names in parentheses is not compared with the machine's: Windows has no IANA zone name without a new dependency.
- A read is a `claude` process for about 1.5 s. An agent update waits for running reads after marking Claude Code as updating, which refuses new ones.
- `/usage` prints the same cost summary for an API-key sign-in and for a profile without any sign-in, so both read as „kein Abo-Kontingent“; the profile row's sign-in dot tells them apart.
- The CLI writes its own state into the profile (`.claude.json`, a `projects` entry for the app data dir), as any session does — so a directory that is someone else's is refused („Der Profilordner gehört nicht zu Claude Code.“), and one that does not exist yet reads as not signed in without a process.
- The child inherits Traygent's environment except the removed override variables (and `CLAUDE_CONFIG_DIR`, set or removed per profile), and each read loads the profile's user-scope settings (`settings.json`, its hooks and MCP servers included); project and local settings never load (`--setting-sources user`, which CLI 2.1.332 answers the same with).
- A shell that exited while a grandchild still holds stdout gives what was printed so far after 500 ms; on Windows the remaining tree is then ended through `taskkill /T` on the exited shell's id. Whether that reaches processes whose parent already exited is unverified; a Job Object would, but needs more of `windows-sys` in the core (BACKLOG).

### Neutral

- `quotaReads` stays off by default for Claude Code; the gate still returns before anything is resolved or started.
- No HTTP status any more: `retryAfterSecs` stays unset for Claude Code, the panel's pacing is unchanged.
- Docker sessions read on the host too: their container uses the same host profile (ADR-0012).

## Alternatives Considered

- **Keep the HTTP read** — rejected by the owner: undocumented endpoint, token out of the CLI's file.
- **`--bare`** — rejected: it may change which sign-in the CLI uses.
- **The user profile (or `C:\`) as working directory** — rejected, also as a fallback without an app data dir: there `.claude\settings.json` would load as project settings of a named profile, and any folder could be a project with its own hooks.

## References

- Issue #110; `src-tauri/src/quota/claude_cli.rs`, `quota/claude_usage.rs`, `launch/login.rs → hidden_usage_plan`, `service/launch_prep.rs → prepare_quota_read`, `quota_paths.rs → quota_source`.
