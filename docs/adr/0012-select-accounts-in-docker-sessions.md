# ADR-0012: Select Accounts in Docker Sessions

- **Status:** Accepted
- **Date:** 2026-11-18
- **Decider(s):** owner
- **Tags:** docker, profiles, github, credentials
- **Supersedes:** the local-only profile selection in [ADR-0009](0009-agent-profiles-with-linked-config-directories.md) and listing-only use of the connected GitHub account in [ADR-0005](0005-start-from-a-github-repository.md)

## Context

Claude Docker sessions previously used the default profile. GitHub token forwarding followed the Windows CLI’s active account, which could differ from the account selected in Traygent. The default Linux image included Git and lacked the GitHub CLI.

## Decision

Apply the existing Claude profile selector, profile sync and workspace restart behavior to Docker. Mount the selected home read-write. For a named profile, mount its linked shared folders from the standard home individually so Linux can follow the same sharing rules as Windows.

Use Git for repository operations and gh for GitHub features such as issues and pull requests. When the Linux image and its tools cache lack a working gh executable, install it in the persistent tools directory. Verify the release archive against the pinned SHA-256 for the container architecture before publishing the binary.

When forwarding is enabled, look up the selected GitHub account’s credential in Windows Credential Manager after the host clone. Recheck the switch and account at lookup time. Supply the token by name to Docker as `GH_TOKEN`. Configure Git’s GitHub HTTPS credential helper to use gh for the same account.

## Consequences

- A workspace restart remembers its Claude identity. Existing workspaces without a saved profile start with Standard.
- Git and gh can use either an imported gh login or a personal access token connected to Traygent. The token’s permissions determine access. Switching accounts in the Windows CLI leaves Traygent’s selection in effect.
- Host clones continue to use Windows Git authentication. Existing containers keep the credentials supplied at launch.
- Container processes and users with access to the Docker daemon can read the forwarded credential. Forwarding defaults to off. A missing or changed account or a missing credential stops an opted-in launch.
