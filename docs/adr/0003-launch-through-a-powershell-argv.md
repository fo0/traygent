# ADR-0003: Launch Through a PowerShell Argv, Never a cmd String

- **Status:** Accepted
- **Date:** 2026-11-03
- **Decider(s):** both
- **Tags:** launch, security, windows

## Context

A start carries user-controlled values into a new terminal: the folder path, a free model name, extra CLI arguments, an OpenCode config JSON and, optionally, a GitHub token. The Batch starters built `cmd.exe` strings, where `& | < > ^ %` in any value become live. The session must open in Windows Terminal (`wt.exe`) or a plain console, and the terminal runs **Windows PowerShell 5.1** (`powershell.exe`), which is on every Windows host.

PowerShell 5.1's native-argument passing has sharp edges: an embedded `"` is not escaped (an argument with a space and a `"` arrives with the quotes stripped), empty arguments are dropped, and an argument containing a space that ends in `\` breaks CRT parsing (`"C:\a b\"`). `wt.exe` splits its command line at every `;`, even inside quotes, and may expand `%VAR%`; whether it unescapes `\;` is unverified. npm `.cmd` shims re-parse their arguments through `cmd.exe` (BatBadBut class). Windows also resolves some paths to something other than a folder: device and namespace prefixes (`\\?\`, `\\.\`), alternate data streams (`C:\a:b`) and reserved device names (`CON`, `NUL`, `COM1` …).

## Decision

We will build every launch as an argv (`Vec<String>`) in a pure builder (`src-tauri/src/launch.rs`) and spawn it directly (`spawn.rs`), never through a shell string. The terminal program is `wt.exe new-tab [-d <folder>] --title <title> powershell.exe …` or `powershell.exe` in a new console; PowerShell runs `-NoLogo -NoExit -EncodedCommand <Base64 of the UTF-16LE script>`, so no `;`, `%`, quote or space of the script reaches wt.exe or PowerShell's argument parsing and no `\;` escaping is needed. wt gets no `-d` for a folder containing `;` or `%` (the process cwd and `Set-Location` still apply), and the tab title has `;` replaced by `,` and `%` by `％`. An encoded command over 30000 characters (Windows caps a command line at 32767) fails with a German error. Inside the script every value is a single-quoted literal with all PowerShell single-quote characters doubled (`launch/quote.rs → ps_literal`), the folder is entered with `Set-Location -LiteralPath '<folder>' -ErrorAction Stop`, so a failed change of directory ends the script before the agent starts, and the CLI is called by absolute path with `&`. No native argv element contains `"` or is empty; folders lose a trailing `\`. Folders must be absolute; device and namespace prefixes, a `:` after the drive (alternate data streams), `< > " | ? *` and reserved device names as a path segment are refused (`launch/folder.rs`). CLI, docker.exe and gh.exe paths must be Windows-absolute; relative or unexpanded (`%`) PATH entries are never searched. JSON and shell scripts travel as Base64 and are decoded into `$env:` variables inside the script (alphabet without `' " ;`), so they never need quoting. Secrets never appear in the script text: the gh token is fetched inside the session and forwarded to Docker by name only (`-e GH_TOKEN`); fetch and `docker` call share one `try { … } finally { [Environment]::SetEnvironmentVariable('GH_TOKEN', $null) }`, so the token leaves the session however the container ends. An `.exe` is preferred over an npm `.cmd` shim; with a shim, any argument containing `& | < > ^ % ! ( )` is refused. If `wt.exe` fails to start, the same launch falls back to a console window.

## Consequences

### Positive

- No injection through a folder name, model or extra argument; the builder is pure and covered by Rust tests on Linux, which decode `-EncodedCommand` and assert the script.
- The token stays out of command lines, window titles, the snapshot and `state.json`.
- Works on any Windows host without PowerShell 7.

### Negative / Trade-offs

- The quoting rules are subtle; a change in the builder needs tests for quotes, `;`, `%`, spaces, trailing `\` and Unicode quotes.
- The command line shows only Base64, so a user inspecting the process sees no readable script; the window title still names agent and folder.
- The script, including the folder path, must stay under the 30000-character cap; long Unicode paths with long extra arguments can hit it.
- A folder containing `;` or `%` cannot use `wt -d` and relies on the process cwd and `Set-Location`.
- That `-ErrorAction Stop` on `Set-Location` ends an `-EncodedCommand` script before the agent call, and that `-EncodedCommand` behaves like `-Command` under `-NoExit`, is not yet verified on Windows (BACKLOG).

### Neutral

- Extra arguments are split on whitespace and must not contain quotes; each becomes one literal.

## Alternatives Considered

- **A `cmd.exe /c` string as in the Batch starters** — rejected: metacharacters become live and quoting is not composable.
- **`-Command <script>` with every `;` escaped as `\;` for wt** — the first draft; rejected: whether wt unescapes `\;` is unverified, wt may still expand `%VAR%`, and the script reaches PowerShell's own argument parsing.
- **PowerShell 7 (`pwsh`)** — rejected: not installed by default; 5.1 is the common denominator.
- **Environment variables on the spawned process** — rejected: `wt.exe` may hand the tab to an already running Windows Terminal whose environment is not Traygent's.

## References

- `src-tauri/src/launch.rs`, `src-tauri/src/launch/folder.rs`, `src-tauri/src/launch/quote.rs`, `src-tauri/src/spawn.rs`
- `docs/SPEC.md → Launch rules`, `docs/AGENT_RECIPES.md §0`
