import type { Bridge } from './bridgeTypes';
import { MINUTE, minuteClock } from './clock';
import type { QuotaContext } from './quotaContexts';
import { DEFAULT_QUOTA_REFRESH_MINUTES, type AgentQuota } from './quotaTypes';
import type { AgentId } from './types';

// A provider that keeps reporting a reset moments away must not turn into a request loop.
const MIN_INTERVAL = 30_000;
// A failed read keeps the last good windows this long: enough for a network blip,
// short enough that a signed-out account does not keep its old figures.
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

interface Entry {
  context: QuotaContext;
  result?: AgentQuota;
  busy: boolean;
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
 * can only update its own entry.
 */
export class AgentQuotas {
  now = $state(Date.now());
  #read: Bridge['getAgentQuota'];
  #cache = new Map<string, Entry>();
  #selected = new Map<string, Entry>();
  #lead = new Map<AgentId, string>();
  #observing = false;
  #expiry: ReturnType<typeof setTimeout> | undefined;
  #interval = DEFAULT_QUOTA_REFRESH_MINUTES * MINUTE;
  #shown = $state.raw<Shown>({ entries: {}, lead: {} });

  constructor(read: Bridge['getAgentQuota']) {
    this.#read = read;
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
        context,
        busy: false,
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

  refresh(force = false) {
    if (!force && typeof document !== 'undefined' && document.hidden) return;
    const now = Date.now();
    this.now = now;
    for (const entry of this.#selected.values()) {
      if (!entry.busy && (force || now >= entry.nextRead))
        void this.#load(entry);
    }
    this.#scheduleExpiry(now);
    for (const [key, entry] of this.#cache) {
      if (!this.#isSelected(entry) && !entry.busy && entry.nextRead <= now) {
        this.#cache.delete(key);
      }
    }
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
    entry.busy = true;
    const { agent, env, profile } = entry.context;
    let result: AgentQuota;
    try {
      result = (await this.#read(agent, profile, env)) ?? unavailable;
    } catch {
      result = unavailable;
    }
    // A failed read (no windows, only a detail; a switched-off read has neither) keeps the last good
    // windows with the new detail for a while; their own resets still expire them.
    const now = Date.now();
    const previous = entry.result;
    delete entry.staleUntil;
    if (hasWindows(result)) entry.goodAt = now;
    else if (
      result.detail &&
      previous &&
      hasWindows(previous) &&
      entry.goodAt !== undefined &&
      now < entry.goodAt + STALE_LIMIT
    ) {
      entry.staleUntil = entry.goodAt + STALE_LIMIT;
      result = { windows: previous.windows, detail: result.detail };
    }
    entry.result = result;
    entry.busy = false;
    entry.lastRead = now;
    this.#setNextRead(entry);
    if (this.#isSelected(entry)) {
      this.now = now;
      this.#publish();
      this.#scheduleExpiry(now);
    }
  }

  #setNextRead(entry: Entry) {
    const at = entry.lastRead;
    if (at === undefined) return;
    entry.nextRead = Math.max(
      at + MIN_INTERVAL,
      Math.min(
        at + this.#interval,
        ...resets(entry.result).filter((reset) => reset > at),
      ),
    );
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
      stopClock();
      window.removeEventListener('focus', tick);
    };
  }
}
