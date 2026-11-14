/*
 * Synthetic profiles of the browser preview (#33, #41): Claude Code's
 * „Standard“, signed in with an invented `example.com` account, and „Team“,
 * signed out until „Anmelden“; a demo sign-in reports the same account with
 * an organization of its own, like one person in two organizations. Also the
 * extra names they share (#35). Folders and times are invented and never
 * read from the machine.
 */
import { DEMO_HOME } from './demoRecents';
import { DEFAULT_PROFILE, PROFILE_MISSING, STANDARD_LABEL } from './profiles';
import type {
  AgentCatalogEntry,
  AgentId,
  AgentProfile,
  AppSettings,
  ProfileShare,
  ProfileStatus,
  Snapshot,
} from './types';

/** The demo's extra profiles per agent. */
export function demoProfileSettings(): Record<AgentId, AgentProfile[]> {
  return { claude: [{ id: 'team', label: 'Team' }], opencode: [], codex: [] };
}

/** The demo's extra shared names: Claude Code shares one of each. */
export function demoProfileShare(): Record<AgentId, ProfileShare> {
  return {
    claude: { dirs: ['.caveman-sessions'], files: ['orca-state.json'] },
    opencode: { dirs: [], files: [] },
    codex: { dirs: [], files: [] },
  };
}

/** The invented account every demo sign-in reports. */
export const DEMO_PROFILE_ACCOUNT = 'demo@example.com';

/** The invented organization a demo sign-in of a profile reports. */
export const demoOrganization = (id: string, label: string) =>
  id === DEFAULT_PROFILE ? 'Beispiel GmbH' : `Beispiel ${label}`;

/**
 * The statuses of the saved profiles, „Standard“ first; a profile already
 * listed keeps its sign-in and sync, a new one starts signed out.
 */
export function demoProfiles(
  settings: Pick<AppSettings, 'profiles'>,
  catalog: readonly AgentCatalogEntry[],
  previous: readonly ProfileStatus[],
): ProfileStatus[] {
  return catalog
    .filter((entry) => entry.profiles)
    .flatMap(({ id: agent }) =>
      [
        { id: DEFAULT_PROFILE, label: STANDARD_LABEL },
        ...(settings.profiles[agent] ?? []),
      ].map(({ id, label }) => {
        const known = previous.find((p) => p.agent === agent && p.id === id);
        const dir = `${DEMO_HOME}\\.${agent}${id === DEFAULT_PROFILE ? '' : `-${id}`}`;
        return known
          ? { ...known, label }
          : { agent, id, label, dir, signedIn: false };
      }),
    );
}

/** The demo's first statuses: „Standard“ signed in, „Team“ synced `now`. */
export function demoProfileStatuses(
  settings: Pick<AppSettings, 'profiles'>,
  catalog: readonly AgentCatalogEntry[],
  now: number,
): ProfileStatus[] {
  return demoProfiles(settings, catalog, []).map((status) =>
    status.id === DEFAULT_PROFILE
      ? {
          ...status,
          signedIn: true,
          account: DEMO_PROFILE_ACCOUNT,
          organization: demoOrganization(status.id, status.label),
        }
      : { ...status, syncedAt: new Date(now - 20 * 60_000).toISOString() },
  );
}

/** A finished sync: every extra profile synced `now`, without an error. */
export function syncedProfiles(
  statuses: readonly ProfileStatus[],
  now: number,
): ProfileStatus[] {
  return statuses.map((status) => {
    if (status.id === DEFAULT_PROFILE) return status;
    const synced = { ...status, syncedAt: new Date(now).toISOString() };
    delete synced.syncError;
    return synced;
  });
}

/** A finished demo sign-in of one profile. */
export function signedIn(
  statuses: readonly ProfileStatus[],
  agent: AgentId,
  id: string,
): ProfileStatus[] {
  return statuses.map((status) =>
    status.agent === agent && status.id === id
      ? {
          ...status,
          signedIn: true,
          account: DEMO_PROFILE_ACCOUNT,
          organization: demoOrganization(id, status.label),
        }
      : status,
  );
}

/** What the demo's sign-ins need from the demo bridge. */
export interface DemoSignInHost {
  /** The current statuses. */
  profiles: () => readonly ProfileStatus[];
  /** The saved „Debug“ setting. */
  debug: () => boolean;
  /** Applies new statuses under the next revision. */
  commit: (profiles: ProfileStatus[]) => void;
  /** A copy of the current snapshot. */
  snapshot: () => Snapshot;
  /** Pushes the current snapshot like `traygent:snapshot`. */
  emit: () => void;
}

/**
 * The demo's sign-ins (#41), as natively but without a CLI: hidden, one per
 * profile ends after `wait` ms signed in and can be cancelled; with „Debug“
 * the terminal „opens“ at once and the status follows as an event.
 */
export function demoSignIns(host: DemoSignInHost, wait: number) {
  const running = new Map<string, () => void>();
  const finish = (agent: AgentId, id: string) =>
    host.commit(signedIn(host.profiles(), agent, id));
  return {
    login(agent: AgentId, id: string): Promise<Snapshot> {
      if (!host.profiles().some((p) => p.agent === agent && p.id === id))
        return Promise.reject(new Error(PROFILE_MISSING));
      const key = `${agent}:${id}`;
      if (running.has(key))
        return Promise.reject(new Error('Die Anmeldung läuft bereits.'));
      if (host.debug()) {
        setTimeout(() => {
          finish(agent, id);
          host.emit();
        }, wait);
        return Promise.resolve(host.snapshot());
      }
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          running.delete(key);
          finish(agent, id);
          resolve(host.snapshot());
        }, wait);
        running.set(key, () => {
          clearTimeout(timer);
          running.delete(key);
          reject(new Error('Anmeldung abgebrochen.'));
        });
      });
    },
    /** True when a hidden sign-in of the profile was running. */
    cancel(agent: AgentId, id: string) {
      const stop = running.get(`${agent}:${id}`);
      stop?.();
      return !!stop;
    },
  };
}
