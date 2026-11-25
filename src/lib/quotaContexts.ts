import { quotaReadOn, shownQuotaProfiles } from './quotaTypes';
import { defaultProfile, DEFAULT_PROFILE, profileLabel } from './profiles';
import type { AgentId, EnvId, Snapshot } from './types';

/** One read the quota cache keeps: an agent's profile in an environment. */
export interface QuotaContext {
  agent: AgentId;
  env: EnvId;
  profile: string;
  identity: string;
  /** The profile `read(agent)` answers for; else the agent's first context. */
  lead?: boolean;
}

/** A profile the quota tile shows for an agent. */
export interface TileProfile {
  id: string;
  /** The profile's label; `''` for an agent without extra profiles. */
  name: string;
  /** The profile a start begins with: always shown, its name emphasized. */
  start: boolean;
}

/**
 * The profiles the tile shows for `agent` (#94): with a `startProfile` (the
 * selected agent) every profile `shownQuotaProfiles` names, in list order,
 * else the agent's saved default alone. Names only when the agent has a
 * profile besides „Standard“.
 */
export function tileProfiles(
  snapshot: Pick<Snapshot, 'settings' | 'profiles'>,
  agent: AgentId,
  startProfile?: string,
): TileProfile[] {
  const { settings } = snapshot;
  const known = [
    DEFAULT_PROFILE,
    ...(settings.profiles?.[agent] ?? []).map((p) => p.id),
  ];
  const start = startProfile ?? defaultProfile(settings, agent);
  const ids =
    startProfile === undefined
      ? [start]
      : shownQuotaProfiles(settings, agent, start, known);
  const named = known.length > 1;
  return ids.map((id) => ({
    id,
    name: named ? profileLabel(snapshot, agent, id) : '',
    start: id === start,
  }));
}

/**
 * The reads the panel keeps: every shown profile of the selected agent, the
 * saved default of each other agent whose read is on.
 */
export function quotaContexts(
  snapshot: Snapshot,
  active: AgentId,
  selectedProfile: string,
  env: EnvId,
): QuotaContext[] {
  const { settings, profiles, catalog } = snapshot;
  // An agent whose read is off gets no context, so the provider is never asked.
  const reading = catalog.filter(({ id }) => quotaReadOn(settings, id));
  return reading.flatMap(({ id: agent }) =>
    tileProfiles(
      snapshot,
      agent,
      agent === active ? selectedProfile : undefined,
    ).map(({ id: profile, start }) => {
      const status = profiles?.find(
        (p) => p.agent === agent && p.id === profile,
      );
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
      return { agent, env, profile, identity, lead: start };
    }),
  );
}
