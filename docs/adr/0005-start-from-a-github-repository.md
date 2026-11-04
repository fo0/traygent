# ADR-0005: Start a Session from a GitHub Repository

- **Status:** Accepted; partly superseded — default clone root and the restart folder by [ADR-0006](0006-default-clone-root-in-a-traygent-folder.md)
- **Date:** 2026-11-04
- **Decider(s):** owner
- **Tags:** launch, github, auth, ui

## Context

A session starts in a folder (`docs/SPEC.md`). Most projects live in a GitHub repository, often not yet cloned on this PC. The owner wants the composer's folder field to take a repository too (#13): a pasted folder path or repository URL is recognised, and typed text searches the repositories of the user's GitHub account. RepoWatch (`fo0/repo-watch`) already connects a GitHub account with a personal access token or by importing the GitHub CLI (`gh`) sign-in, and searches the account's repository listing locally (its ADR-0006).

## Decision

We will let the folder field classify its text, in the panel while typing and in Rust as the authority:

- an absolute Windows path (`C:\…`, `\\server\…`) is a **folder**, as before;
- `https://github.com/<owner>/<repo>` (optional `.git`, trailing path or `/`), `git@github.com:<owner>/<repo>(.git)` and a bare `<owner>/<repo>` are a **repository**;
- anything else is **search text**: the repositories of the connected account whose `owner/name` contains every word are suggested below the field, newest push first.

A start with a repository sends `repo: { owner, name }` instead of a folder _(superseded by ADR-0006)_. Rust derives the folder `<clone root>\<owner>\<name>` (clone root from Settings → GitHub, default `%USERPROFILE%\source\repos` _(superseded by ADR-0006)_). An existing folder is reused as it is; otherwise the session's own PowerShell script runs `git clone -- https://github.com/<owner>/<name>.git <folder>` before the agent, every argument a single-quoted literal (ADR-0003), and stops when the clone fails. The clone uses the user's Git credentials (Git Credential Manager); the GitHub token never reaches a command line, an environment variable or the clone. A Docker session clones on the host and bind-mounts the folder as usual. The recent session keeps the folder and the repository.

We will connect **one** GitHub account, ported from RepoWatch: a personal access token, or the token of a local `gh` sign-in imported after an explicit „Übernehmen“. The token is verified against `/user` and stored in the operating system's credential vault (`keyring`), never in `state.json`; `state.json` keeps only login, user id and source. The account's `/user/repos` listing (owner, collaborator, organisation member; at most 1000, newest push first) is cached in memory, read at startup, after connecting and on demand, and filtered locally.

## Consequences

### Positive

- One field for both: paste a path, paste a URL, or type a few letters of a repository name.
- No clone step outside the terminal: progress, credential prompts and errors are visible where the session runs.
- The GitHub token only reads the listing; cloning works without an account for every repository Git itself can reach.

### Negative / Trade-offs

- A new runtime dependency (`keyring`) and a second credential copy next to `gh`'s; a token rotated in `gh` must be imported again.
- The listing can be up to 15 minutes old; a repository created since is found only after „Aktualisieren“ or by typing `owner/name`.
- Repositories outside the account's listing are suggested only when typed as `owner/name` or URL.

### Neutral

- A reused folder is not checked to be a clone of that repository; it is the user's folder.
- GitHub Enterprise hosts are not recognised.

## Alternatives Considered

- **Clone in Rust before the terminal opens** — not chosen: a long clone would block the start with no progress and credential prompts would have nowhere to appear.
- **Clone with the stored token** — not chosen: it would put the token into a URL, an environment variable or a credential helper of the session.
- **GitHub search API per keystroke** — not chosen (as RepoWatch ADR-0006): latency and rate limits; the listing is local.
- **Several accounts as in RepoWatch** — not chosen: one account covers the search; several would triple the account UI.

## References

- #13, `agent_docs/ux_rules.md`, [ADR-0003](0003-launch-through-a-powershell-argv.md)
- RepoWatch: `docs/LOCAL_AUTH.md`, `docs/adr/0006-search-reaches-every-repository.md`, `src-tauri/src/local_auth.rs`
