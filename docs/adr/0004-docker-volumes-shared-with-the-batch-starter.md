# ADR-0004: Docker Sessions Share the Batch Starter's Claude Volumes

- **Status:** Accepted
- **Date:** 2026-11-03
- **Decider(s):** both
- **Tags:** docker, launch, storage

## Context

Users of `fo0/agent-tools` already run Claude Code in Docker through `claude/cli-starter.bat`, with their login, settings and the installed CLI in the named volumes `claude-code-tools` (`/home/node/.local`) and `claude-code-config` (`/home/node/.claude-volume`, `CLAUDE_CONFIG_DIR=…/config`, `~/.claude` a symlink to it). agent-tools ADR-0007 keeps agent state in named volumes, never bind-mounts; its ADR-0009 explains why the config volume must not be mounted at `~/.claude`. A second login, or a second copy of the CLI, would be friction for exactly the people Traygent is for.

The OpenCode starter builds its own image bound to a company LLM gateway, and there is no Codex starter. OpenCode and Codex keep their session lists per working directory, so every folder mounted at the same `/workspace` would share one list and `--continue` would pick another project's session.

## Decision

We will run every Docker session from a plain `node:22-trixie` image (changeable per agent) with `docker run --rm -it --init`, the folder bind-mounted and all state in named volumes (`src-tauri/src/launch/docker.rs`). **Claude Code** reuses the starter's volumes, paths, environment and per-folder project dir name (`CLAUDE_CODE_PROJECT_DIR_NAME`, byte for byte the starter's algorithm), and mounts the folder at `/workspace`, so a login carries over in both directions. Unlike the starter it does not set `CLAUDE_CODE_TRUST_WORKSPACE`: the trust prompt appears once per folder, and skipping it would let a repository's hooks run unasked. **OpenCode** uses `traygent-opencode-bin`, `traygent-opencode-data`, `traygent-opencode-config`; **Codex** uses `traygent-codex-tools`, `traygent-codex-home`. Both mount the folder at `/workspace/<pdn>` (the same project dir name). All three run with `--security-opt no-new-privileges` (root prepares, `runuser` drops to `node`, neither needs setuid; UNVERIFIED for Claude on a real engine). The image is validated when `state.json` is read and again before it reaches `docker run` or `docker image inspect`. On first start an init script, passed as `TRAYGENT_INIT` in Base64 and sourced by a fixed loader, installs the CLI into its tools volume as user `node`.

## Consequences

### Positive

- An existing Claude container login keeps working; Traygent and the Batch starter can be used side by side.
- One login per agent for all folders; per-folder session lists for OpenCode and Codex.
- No image build step and no gateway coupling; image upgrades do not touch state.

### Negative / Trade-offs

- Traygent is bound to the starter's Claude layout; a change there must be mirrored here.
- The first start per agent pulls the image and installs the CLI, which takes a while; parallel first starts may race the installer (unverified impact).
- The tools and config volumes are shared and writable across all workspaces: a session compromised by one repository (a malicious hook, a poisoned dependency) can replace the CLI binary or its settings, and that persists into every later session of that agent, in every folder, including the Batch starter's. Future work: mount the tools volume read-only in sessions and install or update the CLI in a separate updater container; keep per-repository config out of the shared volume where the agent allows it.
- Without `CLAUDE_CODE_TRUST_WORKSPACE` every folder asks for trust once on its first Claude Docker start.
- Features the starter has and Traygent does not yet (corporate CA, GPU, port publishing) stay with the starter.

### Neutral

- OpenCode and Codex volumes carry a `traygent-` prefix, so they never collide with the starter's.

## Alternatives Considered

- **Traygent-owned volumes for Claude too** — rejected: forces a second login and a second CLI install.
- **Reuse the OpenCode starter's image** — rejected: needs a build step and is bound to one company's gateway.
- **One `/workspace` for every agent** — rejected for OpenCode and Codex: their per-cwd session lists would mix projects.
- **Bind-mount the host's config directories** — rejected: agent-tools ADR-0007 keeps agent state in named volumes.

## References

- `src-tauri/src/launch/docker.rs`, `src-tauri/src/detect.rs → probe_docker_agents`
- `docs/AGENT_RECIPES.md §1–§3 (… in Docker)`, agent-tools ADR-0007 / ADR-0009
