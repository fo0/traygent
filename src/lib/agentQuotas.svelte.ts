import type { Bridge } from './bridgeTypes';
import { MINUTE, minuteClock } from './clock';
import { clockTime } from './format';
import type { QuotaContext } from './quotaContexts';
import { Backoff, ProviderQueue, rateLimited } from './quotaSchedule';
import { DEFAULT_QUOTA_REFRESH_MINUTES, type AgentQuota } from './quotaTypes';
import type { AgentId } from './types';

// A provider that keeps reporting a reset moments away must not turn into a request loop.
const MIN_INTERVAL = 30_000;
// A failed read keeps the last good windows this long: enough for a network blip,
// short enough that a signed-out account does not keep its old figures. A read the
// provider limited (429, #100) says nothing about the account: windows with a reset
// stay until it; only those without one follow this limit.
const STALE_LIMIT = 15 * 60_000;
const unavailable: AgentQuota = {
  windows: {},
  detail: 'Kontingent konnte nicht geladen werden.',
};

const hasWindows = (quota?: AgentQuota) =>
  Object.keys(quota?.windows ?? {}).length > 0;

const resets = (quota?: AgentQuota) =>
  Object.values(quota?.windows ?? {})
    .map((window) => Date.parse(window.resetAt ?? ''))
    .filter((reset) => !Number.isNaN(reset));

const hidden = () => typeof document !== 'undefined' && document.hidden;

interface Entry {
  /** The account: agent, environment, profile and sign-in. */
  key: string;
  context: QuotaContext;
  result?: AgentQuota;
  busy: boolean;
  /** Due, waiting for its provider to be free (#100). */
  queued: boolean;
  nextRead: number;
  lastRead?: number;
  goodAt?: number;
  staleUntil?: number;
}

/** Where an agent's profile sits among the shown readings. */
const slot = (agent: AgentId, profile: string) =>
  JSON.stringify([agent, profile]);

interface Shown {
  entries: Record<string, Entry>;
  /** Per agent, the profile `read(agent)` answers for. */
  lead: Partial<Record<AgentId, string>>;
}

/**
 * Cache by launch account, one entry per shown profile (#94). A late reply
 * can only update its own entry. Reads of one provider run one at a time,
 * paced and backed off by `quotaSchedule.ts` (#100).
 */
export class AgentQuotas {
  now = $state(Date.now());
  #read: Bridge['getAgentQuota'];
  #cache = new Map<string, Entry>();
  #selected = new Map<string, Entry>();
  #lead = new Map<AgentId, string>();
  #observing = false;
  #expiry: ReturnType<typeof setTimeout> | undefined;
  #pumpTimer: ReturnType<typeof setTimeout> | undefined;
  #queue: ProviderQueue;
  #backoff = new Backoff();
  #interval = DEFAULT_QUOTA_REFRESH_MINUTES * MINUTE;
  #shown = $state.raw<Shown>({ entries: {}, lead: {} });

  /** `random` (0–1) spreads the reads of one provider; tests pin it. */
  constructor(read: Bridge['getAgentQuota'], random?: () => number) {
    this.#read = read;
    this.#queue = new ProviderQueue(random);
  }

  #entry(agent: AgentId, profile?: string) {
    const { entries, lead } = this.#shown;
    const id = profile ?? lead[agent];
    return id === undefined ? undefined : entries[slot(agent, id)];
  }

  /** The reading of `agent`'s `profile`; without one, of its lead profile. */
  read(agent: AgentId, profile?: string) {
    const entry = this.#entry(agent, profile);
    if (entry?.staleUntil && this.now >= entry.staleUntil && entry.result)
      return { windows: {}, detail: entry.result.detail };
    return entry?.result;
  }

  updatedAt(agent: AgentId, profile?: string) {
    return this.#entry(agent, profile)?.goodAt;
  }

  follow(contexts: QuotaContext[], minutes = DEFAULT_QUOTA_REFRESH_MINUTES) {
    if (this.#interval !== minutes * MINUTE) {
      this.#interval = minutes * MINUTE;
      for (const entry of this.#cache.values()) this.#setNextRead(entry);
    }
    const next = new Map<string, Entry>();
    const lead = new Map<AgentId, string>();
    for (const context of contexts) {
      const { agent, env, profile, identity } = context;
      const key = JSON.stringify([agent, env, profile, identity]);
      const entry = this.#cache.get(key) ?? {
        key,
        context,
        busy: false,
        queued: false,
        nextRead: 0,
      };
      this.#cache.set(key, entry);
      next.set(slot(agent, profile), entry);
      if (context.lead || !lead.has(agent)) lead.set(agent, profile);
    }
    this.#selected = next;
    this.#lead = lead;
    // A switched-off agent keeps nothing cached: switching it back on reads afresh.
    for (const [key, entry] of this.#cache)
      if (!lead.has(entry.context.agent)) this.#cache.delete(key);
    this.#publish();
    this.refresh();
  }

  #isSelected(entry: Entry) {
    const { agent, profile } = entry.context;
    return this.#selected.get(slot(agent, profile)) === entry;
  }

  #publish() {
    const entries: Record<string, Entry> = {};
    for (const [at, entry] of this.#selected) {
      if (entry.result) entries[at] = entry;
    }
    this.#shown = { entries, lead: Object.fromEntries(this.#lead) };
  }

  /** `force` skips the regular interval, never the queue or a backoff (#100). */
  refresh(force = false) {
    if (!force && hidden()) return;
    const now = Date.now();
    this.now = now;
    for (const entry of this.#selected.values()) {
      if (
        !entry.busy &&
        now >= this.#backoff.until(entry.key) &&
        (force || now >= entry.nextRead)
      )
        entry.queued = true;
    }
    for (const [key, entry] of this.#cache) {
      if (this.#isSelected(entry)) continue;
      entry.queued = false;
      if (!entry.busy && entry.nextRead <= now) this.#cache.delete(key);
    }
    this.#pump(now);
    this.#scheduleExpiry(now);
  }

  /** Starts each provider's next queued read once it is free; wakes up for the rest. */
  #pump(now: number) {
    clearTimeout(this.#pumpTimer);
    let wake = Infinity;
    for (const entry of this.#selected.values()) {
      if (!entry.queued || entry.busy) continue;
      const free = this.#queue.freeAt(entry.context.agent);
      if (free > now) wake = Math.min(wake, free);
      else {
        entry.queued = false;
        void this.#load(entry);
      }
    }
    // In flight (`Infinity`): its end pumps. A hidden panel starts nothing more
    // until the next refresh.
    if (wake < Infinity)
      this.#pumpTimer = setTimeout(() => {
        if (!hidden()) this.#pump(Date.now());
      }, wake - now);
  }

  // The shared clock drives countdowns. A reported window ending between ticks
  // gets its own wake-up so a spent allowance disappears at its actual reset,
  // even when the next provider read is held back by the minimum interval.
  #scheduleExpiry(now: number) {
    clearTimeout(this.#expiry);
    if (!this.#observing) return;
    const next = Math.min(
      ...[...this.#selected.values()]
        .flatMap((e) => [e.nextRead, e.staleUntil ?? 0, ...resets(e.result)])
        .filter((t) => t > now),
    );
    if (next - now < MINUTE)
      this.#expiry = setTimeout(() => this.refresh(), next - now);
  }

  async #load(entry: Entry) {
    const { agent, env, profile } = entry.context;
    entry.busy = true;
    this.#queue.start(agent);
    let result: AgentQuota;
    try {
      result = (await this.#read(agent, profile, env)) ?? unavailable;
    } catch {
      result = unavailable;
    }
    const now = Date.now();
    this.#queue.finish(agent, now);
    entry.result = this.#settle(entry, result, now);
    entry.busy = false;
    entry.lastRead = now;
    this.#setNextRead(entry);
    if (this.#isSelected(entry)) {
      this.now = now;
      this.#publish();
      this.#scheduleExpiry(now);
    }
    if (!hidden()) this.#pump(now);
  }

  // A failed read (no windows, only a detail; a switched-off read has neither) keeps the last good
  // windows with the new detail for a while; their own resets still expire them.
  #settle(entry: Entry, result: AgentQuota, now: number): AgentQuota {
    const previous = entry.result?.windows ?? {};
    const shown =
      hasWindows(entry.result) &&
      !(entry.staleUntil !== undefined && now >= entry.staleUntil);
    const fresh =
      entry.goodAt !== undefined && now < entry.goodAt + STALE_LIMIT;
    delete entry.staleUntil;
    if (hasWindows(result)) {
      entry.goodAt = now;
      this.#backoff.clear(entry.key);
      return result;
    }
    if (rateLimited(result)) {
      this.#backoff.limited(entry.key, now, result.retryAfterSecs);
      const windows = Object.fromEntries(
        Object.entries(shown ? previous : {}).filter(
          ([, window]) => window?.resetAt != null || fresh,
        ),
      );
      // `#setNextRead` names the next attempt in the detail.
      return { ...result, windows };
    }
    if (result.detail && shown && fresh) {
      entry.staleUntil = entry.goodAt! + STALE_LIMIT;
      return { windows: previous, detail: result.detail };
    }
    return result;
  }

  #setNextRead(entry: Entry) {
    const at = entry.lastRead;
    if (at === undefined) return;
    entry.nextRead = Math.max(
      at + MIN_INTERVAL,
      this.#backoff.until(entry.key),
      Math.min(
        at + this.#interval,
        ...resets(entry.result).filter((reset) => reset > at),
      ),
    );
    // The regular interval can put the next read past the backoff (#100).
    if (rateLimited(entry.result))
      entry.result = {
        ...entry.result,
        detail: `Zu viele Abfragen. Nächster Versuch um ${clockTime(entry.nextRead)}.`,
      };
  }

  /** Timers pause provider reads while hidden. Reopening checks freshness right away. */
  observe() {
    const tick = () => this.refresh();
    this.#observing = true;
    const stopClock = minuteClock.subscribe(tick);
    window.addEventListener('focus', tick);
    return () => {
      this.#observing = false;
      clearTimeout(this.#expiry);
      clearTimeout(this.#pumpTimer);
      stopClock();
      window.removeEventListener('focus', tick);
    };
  }
}
