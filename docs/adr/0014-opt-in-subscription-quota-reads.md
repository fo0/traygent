# ADR-0014: Opt-In Subscription Quota Reads

- **Status:** Accepted
- **Date:** 2026-11-22
- **Deciders:** Owner and implementation agent
- **Tags:** quota, network, privacy, settings

## Context

The composer shows the remaining subscription quota per agent (#83). For Claude Code and Codex, Traygent reads it from `api.anthropic.com/api/oauth/usage` and `chatgpt.com/backend-api/wham/usage`. Neither endpoint is documented for third-party use. Traygent queries them with the access token from the CLI's own subscription sign-in (`.credentials.json`, `auth.json`), reused read-only: it never refreshes the token, never stores it and wipes its copy after the request. No statement from either provider permitting this was found. OpenQuota, the reference implementation the reads were modelled on, states none either. An earlier `User-Agent: claude-code/…` override, which made the request look like the CLI's own, was removed in 42b162e.

OpenCode Go is different: its usage endpoint is queried with the user's own OpenCode Go API key, read from the OpenCode database.

The owner wants no terms-of-service risk by default.

## Decision

We will make every quota read opt-in per agent through the setting `quotaReads` (`PerAgent<bool>`). It defaults to off for Claude Code and Codex and on for OpenCode Go. A state file without the field loads these defaults. „Einstellungen → Agenten → Kontingent“ carries the staged switch „Kontingent abfragen“; its tooltip says which sign-in the read uses and, for Claude Code and Codex, that the provider interface is not officially documented. With the switch off, the panel never calls `get_agent_quota` for that agent and drops its cached reading; the composer button shows no figure and no bar. Rust enforces the same: `quota::read` returns an empty result before the credential path is resolved, so no sign-in file or database is opened and no request is built. Requests that do run carry Traygent's own `User-Agent` only, and a scheduled refresh never follows the previous read of the same account by less than 30 seconds (a manual refresh reads at once).

## Consequences

### Positive

- A fresh install sends nothing to an undocumented Claude or Codex endpoint and touches no CLI sign-in for it.
- The user turns a read on knowingly; the tooltip names the risk without a paragraph in the dialog.
- Enforced in Rust too, so a stale or faulty panel cannot trigger a read that is switched off.

### Negative / Trade-offs

- Claude Code and Codex users see no quota until they find and turn on the switch.
- Turning a read on does not make the endpoint documented; the risk moves to the user's informed choice.

### Neutral

- The bar choices stay in place while a read is off and keep their saved order.
- `get_agent_quota` keeps its contract; „off“ is an `AgentQuota` with no windows and no detail.

## Alternatives Considered

- **Remove the Claude and Codex reads** — rejected because the owner wants the feature available for users who accept the risk.
- **Keep the reads on by default** — rejected because it exposes every user to the terms-of-service risk without a choice.

## References

- Issue #83; commit 42b162e (CLI `User-Agent` removed, refresh interval bounded).
- Issue #100: reads paced per provider and a per-account 429 backoff honouring `Retry-After` (`src/lib/quotaSchedule.ts`).
- `src-tauri/src/quota.rs → read`, `src-tauri/src/quota/gate_tests.rs`, `src/lib/quotaContexts.ts → quotaContexts` (in `agentQuotas.svelte.ts` until #94), `src/lib/QuotaSettings.svelte`.
