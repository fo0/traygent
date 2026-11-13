# Scratchpad — Short-Term

Temporary working context. **Clean up aggressively — delete when resolved.**

## Current Work

- **Orca objective** — #37: owner feedback on profiles — profile editable on a „Zuletzt“ entry, adding a profile applies at once, profile details in place, built-in shared names visible, background sync every 15 minutes (2026-11-13). Out of scope: profiles for Codex / OpenCode and Docker (BACKLOG #53, #54), immediate removal, the native Windows checks (BACKLOG #52). Worktrees `tmp/wt-traygent-{recent-profile,profile-settings,auto-sync}` beside the repo, branches `wt/*`, merged into `claude/busy-davinci-d22z74`.
- **Profile sync parity** — #35 / PR #36: `settings.json` and the `.claude.json` subset merge three-way against `.traygent-base`, sign-in keys per profile, `hooks` / `todos` / `session-env` shared, `settings.profileShare` extras ([ADR-0010](docs/adr/0010-three-way-merge-of-the-profiles-settings-and-state-file.md), `docs/SPEC.md → Profiles`). Linux gates only; Windows: `docs/VALIDATION.md` items 17–19, BACKLOG #52, #67; follow-ups BACKLOG #62–#65, #67–#72. (2026-11-13)
- **Agent profiles** — #33 / PR #34: Claude Code profiles with junction-linked config dirs and file sync ([ADR-0009](docs/adr/0009-agent-profiles-with-linked-config-directories.md), `docs/SPEC.md → Profiles`). Linux gates only; nothing ran on Windows — BACKLOG #52, `docs/VALIDATION.md` items 12–16. (2026-11-13)
- **Agent CLI auto-update** — #16: `settings.agentUpdates` (default off), Settings → Updates → „Agenten“, `update_agents`, schedule thread, in-use check through PowerShell CIM ([ADR-0008](docs/adr/0008-background-updates-of-the-agent-clis.md)). Linux gates green; nothing ran on Windows — BACKLOG #46. Follow-up: open sessions in „Zuletzt“ (#17, BACKLOG #47). (2026-11-10)
- **Composer UX polish** — #5: collapsing agent row, GitHub menu in the folder field, `Dropdown` / `ModelCombo` / `ChoiceList` replace every native select and datalist (composer chips, Settings → Agenten, „Bearbeiten …“). Linux gates green; WebView2 look unchecked on Windows. (2026-11-07)
- **RepoWatch parity** — merged with PR #18 (Closes #17) on 2026-11-04 without GitHub CI on its last commits (GitHub incident; owner-ordered merge, local gates green). Open: the native Windows checks BACKLOG #41–#44. (2026-11-04)
- **Start from a GitHub repository** — PR #14 (Closes #13): folder field takes a folder or a GitHub repository with suggestions from the connected account, clone-or-reuse under the clone root, one account (token or gh import, token in the OS vault), Settings → GitHub; contract in ADR-0005 and `docs/SPEC.md → Repositories`. Vitest (376) and `cargo test --features updates` (250) green on Linux; reviews in progress. Next: the native Windows checks — BACKLOG #33 (clone in a wt tab: `$LASTEXITCODE`/`throw`, Git Credential Manager prompt, profile path with spaces) and #35 (real gh import with ≥ 2.40, `--json` and a legacy gh, Credential Manager round trip), then `docs/VALIDATION.md` items 8–11. (2026-11-04)
- **Earlier native checks** — still open before the first release: BACKLOG Validation #12, #14–#18, #20–#22, #25, #27–#29. (2026-11-03)

## Open Questions

_(none)_
