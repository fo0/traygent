# ADR-0013: Prepare Linux Node for Docker sessions

- **Status:** Accepted
- **Date:** 2026-11-18
- **Deciders:** Owner and implementation agent
- **Supersedes:** The direct user settings mount in ADR-0011 when Claude hooks require translation

## Context

Claude reads the selected Windows profile in Docker. Exec-form hooks can name both the Windows NVM executable and a script by absolute drive paths. Claude tries to spawn that executable on Linux. Different projects and hooks can require different Node releases.

## Decision

Use mise inside the Linux container for the optional Node runtime. The app packages its preparation scripts with the executable and mounts them read-only. A numeric project version or LTS selector chooses the shell runtime. Exact hook pins are prepared before Claude starts measuring hook timeouts. Downloads and runtime selection run as the session user, outside the workspace, with mise configuration loading disabled. The first download of mise verifies the release digest for the CPU architecture. [mise installation](https://mise.jdx.dev/installing-mise.html)

Translate exec-form Node hooks in a session-specific copy of the selected profile's user settings. Preserve the separate executable and argument vector and keep the original hook policy and scope. Only paths within the mounted workspace and profile have container equivalents. Traygent mounts the copy over the user settings file when translation is necessary and removes the temporary file when the launcher exits. The rest of the profile, including credential refreshes, follows ADR-0011 and ADR-0012. [Claude Code hook reference](https://code.claude.com/docs/en/hooks#exec-form-and-shell-form)

## Consequences

The same agent can reuse its Linux tools across launches. Turning management off uses the image's Node and still translates Windows paths in supported hooks. The first uncached version requires network access. If installation fails, a project can start with the image's runtime. A hook that explicitly requires an unavailable version reports that failure under its configured hook policy. Windows NVM paths ending in `vX.Y.Z/node.exe` carry an exact pin. Other Windows Node paths select the session runtime. A projected settings file is read-only in that session. Changes made on Windows become visible on the next launch.
