/*
 * Synthetic profiles of the browser preview (#33): Claude Code's „Standard“
 * and „Team“, both signed in with invented `example.com` accounts, and the
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

/** The invented account a demo sign-in of `id` reports. */
export const demoAccount = (id: string) =>
  `${id === DEFAULT_PROFILE ? 'demo' : id}@example.com`;

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

/** The demo's first statuses: every profile signed in, synced `now`. */
export function demoProfileStatuses(
  settings: Pick<AppSettings, 'profiles'>,
  catalog: readonly AgentCatalogEntry[],
  now: number,
): ProfileStatus[] {
  return demoProfiles(settings, catalog, []).map((status) => ({
    ...status,
    signedIn: true,
    account: demoAccount(status.id),
    ...(status.id !== DEFAULT_PROFILE && {
      organization: 'Beispiel GmbH',
      syncedAt: new Date(now - 20 * 60_000).toISOString(),
    }),
  }));
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
      ? { ...status, signedIn: true, account: demoAccount(id) }
      : status,
  );
}
