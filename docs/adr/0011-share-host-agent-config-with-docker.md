# ADR-0011: Share Host Agent Configuration With Docker

- **Status:** Accepted, partly superseded for Claude user settings by [ADR-0013](0013-prepare-linux-node-for-docker-sessions.md)
- **Date:** 2026-11-18
- **Decider(s):** user
- **Tags:** docker, authentication, storage
- **Supersedes:** the Claude and Codex config storage and Claude workspace path rules in [ADR-0004](0004-docker-volumes-shared-with-the-batch-starter.md) and [ADR-0009](0009-agent-profiles-with-linked-config-directories.md)

## Context

A user already signed in on Windows had to maintain a second account session when launching the same agent in Docker. A credential refresh in either environment applied only to that environment.

## Decision

Bind-mount the host Claude configuration or Codex home read-write for every Docker launch of that agent. Use the same mapping for an existing folder, a freshly cloned repository and a workspace restart. Keep the Linux executables in the existing named tools volumes.

## Consequences

- The container can use and refresh file-based credentials and edit the host settings. The mounted agent directory includes user configuration such as skills and hooks. Commands and paths in that configuration must also work on Linux to run there.
- Each host workspace gets a distinct Linux working directory. Claude uses that path for trust and project-scoped MCP settings, and the explicit project name for transcripts. Codex stores Linux SQLite databases in its tools volume to avoid cross-OS file lock conflicts.
- The Claude default home has a separate global state file. Both that file and the config directory belong to the same host account. A custom Claude config directory contains its own global state file. For the default account, a single-file bind shares the state but its adjacent lock directory lives at a different host path. Simultaneous edits of that global file from Windows and Linux can race.
- A container with this access can change how future local sessions behave. Traygent exposes the selected agent's configuration and its workspace. Windows credential vaults are outside these mounts.
- Existing Docker-only config volumes remain available to their original users. Traygent reads the host configuration directly and does not merge or delete the old volumes. OpenCode retains its existing Docker storage layout.

## Alternatives Considered

Copying credentials into a named volume would leave two sets of tokens to keep in sync after refresh. Mounting the entire Windows user profile would expose unrelated files.
