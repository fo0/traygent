/*
 * Synthetic profiles of the browser preview (#33, #41): Claude Code's
 * „Standard“, signed in with an invented `example.com` account, and „Team“,
 * signed out until „Anmelden“; a demo sign-in reports the same account with
 * an organization of its own, like one person in two organizations. Also the
 * extra names they share (#35). Folders and times are invented and never
 * read from the machine.
 */
import { DEMO_HOME } from './demoRecents';
import { DEFAULT_PROFILE, STANDARD_LABEL } from './profiles';
import type {
  AgentCatalogEntry,
  AgentId,
  AgentProfile,
  AppSettings,
  ProfileShare,
  ProfileStatus,
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

/**
 * The demo's hidden sign-ins (#41): each ends after `wait` ms with
 * `finish()`, one per key at a time; a cancel rejects it as natively.
 */
export function demoLogins(wait: number) {
  const running = new Map<string, () => void>();
  return {
    run<T>(key: string, finish: () => T): Promise<T> {
      if (running.has(key))
        return Promise.reject(new Error('Die Anmeldung läuft bereits.'));
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          running.delete(key);
          resolve(finish());
        }, wait);
        running.set(key, () => {
          clearTimeout(timer);
          running.delete(key);
          reject(new Error('Anmeldung abgebrochen.'));
        });
      });
    },
    cancel(key: string) {
      running.get(key)?.();
    },
  };
}
