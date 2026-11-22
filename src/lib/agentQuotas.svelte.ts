import type { Bridge } from './bridgeTypes';
import { MINUTE, minuteClock } from './clock';
import type { AgentQuota } from './quotaTypes';
import { defaultProfile, DEFAULT_PROFILE } from './profiles';
import type { AgentId, EnvId, Snapshot } from './types';

const REFRESH_INTERVAL = 5 * 60_000;
// A provider that keeps reporting a reset moments away must not turn into a request loop.
const MIN_INTERVAL = 30_000;
const unavailable: AgentQuota = {
  windows: {},
  detail: 'Kontingent konnte nicht geladen werden.',
};

interface QuotaContext {
  agent: AgentId;
  env: EnvId;
  profile: string;
  identity: string;
}

export function quotaContexts(
  snapshot: Snapshot,
  active: AgentId,
  selectedProfile: string,
  env: EnvId,
): QuotaContext[] {
  const { settings, profiles, catalog } = snapshot;
  return catalog.map(({ id: agent }) => {
    const profile =
      agent === active ? selectedProfile : defaultProfile(settings, agent);
    const status = profiles?.find((p) => p.agent === agent && p.id === profile);
    // With its first managed profile, Claude Standard switches from the inherited
    // CLAUDE_CONFIG_DIR to the agent's default home, even if its status looks identical.
    const managedHome =
      agent === 'claude' &&
      profile === DEFAULT_PROFILE &&
      !!settings.profiles?.claude?.length;
    const identity = JSON.stringify([
      managedHome,
      status?.signedIn,
      status?.account,
      status?.organization,
      status?.plan,
    ]);
    return { agent, env, profile, identity };
  });
}

const resets = (quota?: AgentQuota) =>
  Object.values(quota?.windows ?? {})
    .map((window) => Date.parse(window.resetAt ?? ''))
    .filter((reset) => !Number.isNaN(reset));

interface Entry {
  context: QuotaContext;
  result?: AgentQuota;
  busy: boolean;
  nextRead: number;
}

/** Cache by launch account. A late reply can only update its own entry. */
export class AgentQuotas {
  now = $state(Date.now());
  #read: Bridge['getAgentQuota'];
  #cache = new Map<string, Entry>();
  #selected = new Map<AgentId, Entry>();
  #observing = false;
  #expiry: ReturnType<typeof setTimeout> | undefined;
  #shown = $state.raw<Partial<Record<AgentId, AgentQuota>>>({});

  constructor(read: Bridge['getAgentQuota']) {
    this.#read = read;
  }

  read(agent: AgentId) {
    return this.#shown[agent];
  }

  follow(contexts: QuotaContext[]) {
    const next = new Map<AgentId, Entry>();
    for (const context of contexts) {
      const key = JSON.stringify([
        context.agent,
        context.env,
        context.profile,
        context.identity,
      ]);
      const entry = this.#cache.get(key) ?? {
        context,
        busy: false,
        nextRead: 0,
      };
      this.#cache.set(key, entry);
      next.set(context.agent, entry);
    }
    this.#selected = next;
    this.#publish();
    this.refresh();
  }

  #publish() {
    const shown: Partial<Record<AgentId, AgentQuota>> = {};
    for (const [agent, entry] of this.#selected) {
      if (entry.result) shown[agent] = entry.result;
    }
    this.#shown = shown;
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
      if (
        entry !== this.#selected.get(entry.context.agent) &&
        !entry.busy &&
        entry.nextRead <= now
      ) {
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
        .flatMap((e) => [e.nextRead, ...resets(e.result)])
        .filter((t) => t > now),
    );
    if (next - now < MINUTE)
      this.#expiry = setTimeout(() => this.refresh(), next - now);
  }

  async #load(entry: Entry) {
    entry.busy = true;
    const { agent, env, profile } = entry.context;
    try {
      entry.result = (await this.#read(agent, profile, env)) ?? unavailable;
    } catch {
      entry.result = unavailable;
    } finally {
      entry.busy = false;
      const now = Date.now();
      entry.nextRead = Math.max(
        now + MIN_INTERVAL,
        Math.min(
          now + REFRESH_INTERVAL,
          ...resets(entry.result).filter((reset) => reset > now),
        ),
      );
      if (this.#selected.get(agent) === entry) {
        this.now = now;
        this.#publish();
        this.#scheduleExpiry(now);
      }
    }
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
