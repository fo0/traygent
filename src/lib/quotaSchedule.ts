import { MINUTE } from './clock';
import type { AgentQuota } from './quotaTypes';
import type { AgentId } from './types';

/*
 * Pacing of provider reads (#100). One provider sees at most one read of
 * Traygent's at a time, and reads of its accounts start a few seconds apart;
 * an account the provider limited (HTTP 429) waits out a growing backoff.
 */

/** The least pause between the end of one read of a provider and the next. */
export const READ_GAP = 5_000;
/** The most random extra pause, so several accounts never line up again. */
export const READ_JITTER = 2_000;
/** The pause after the 1st, 2nd, … 429 in a row; the last one repeats. */
const BACKOFF = [5, 10, 20, 40, 60].map((minutes) => minutes * MINUTE);
/** The longest `Retry-After` honoured; Rust caps it the same. */
const MAX_RETRY_AFTER = 24 * 60 * MINUTE;

/** A read the provider refused as too frequent. */
export const rateLimited = (
  quota?: AgentQuota,
): quota is AgentQuota & { retryAfterSecs: number } =>
  typeof quota?.retryAfterSecs === 'number';

/**
 * The wait after the `streak`-th 429 in a row (from 1): 5, 10, 20, 40, then
 * 60 minutes, and never shorter than the provider's `Retry-After`.
 */
export function backoffDelay(streak: number, retryAfterSecs = 0): number {
  const step = BACKOFF[Math.min(Math.max(streak, 1), BACKOFF.length) - 1]!;
  const asked = Number.isFinite(retryAfterSecs)
    ? Math.min(Math.max(retryAfterSecs, 0) * 1000, MAX_RETRY_AFTER)
    : 0;
  return Math.max(step, asked);
}

/** Per account (cache key): the 429 streak and when the next read may go. */
export class Backoff {
  #accounts = new Map<string, { streak: number; until: number }>();

  until(key: string) {
    return this.#accounts.get(key)?.until ?? 0;
  }

  /** Counts a 429 for `key` and returns when its next read may start. */
  limited(key: string, now: number, retryAfterSecs?: number) {
    const streak = (this.#accounts.get(key)?.streak ?? 0) + 1;
    const until = now + backoffDelay(streak, retryAfterSecs);
    this.#accounts.set(key, { streak, until });
    return until;
  }

  /** A good read ends the streak. */
  clear(key: string) {
    this.#accounts.delete(key);
  }
}

/** Per provider: at most one read in flight, the next a gap plus jitter later. */
export class ProviderQueue {
  #busy = new Set<AgentId>();
  #freeAt = new Map<AgentId, number>();
  #random: () => number;

  constructor(random: () => number = Math.random) {
    this.#random = random;
  }

  /** When `agent`'s next read may start; `Infinity` while one is in flight. */
  freeAt(agent: AgentId) {
    return this.#busy.has(agent) ? Infinity : (this.#freeAt.get(agent) ?? 0);
  }

  start(agent: AgentId) {
    this.#busy.add(agent);
  }

  finish(agent: AgentId, now: number) {
    this.#busy.delete(agent);
    const jitter = Math.min(Math.max(this.#random(), 0), 1) * READ_JITTER;
    this.#freeAt.set(agent, now + READ_GAP + jitter);
  }
}
