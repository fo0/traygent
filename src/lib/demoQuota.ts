import { quotaReadOn, type AgentQuota, type QuotaMetric } from './quotaTypes';
import type { AgentId, EnvId, Snapshot } from './types';

const HOURS = 3_600_000;
const samples: Record<
  AgentId,
  Partial<Record<QuotaMetric, [number, number]>>
> = {
  claude: {
    week: [32, 72],
    session: [58, 4],
    fable: [81, 72],
    sonnet: [24, 72],
  },
  opencode: { week: [74, 120], session: [41, 4], month: [24, 360] },
  codex: {
    week: [94, 11],
    session: [69, 2],
    spark: [26, 2],
    sparkWeek: [44, 11],
  },
};

/**
 * Invented usage for the browser demo. Reset times stay fixed across
 * refreshes. A switched-off read answers like Rust's gate: nothing.
 */
export function demoQuota(
  agent: AgentId,
  profile: string,
  env: EnvId,
  { settings, profiles }: Pick<Snapshot, 'settings' | 'profiles'>,
  epoch: number,
): AgentQuota {
  if (!quotaReadOn(settings, agent)) return { windows: {}, detail: null };
  if (env === 'wsl' || (agent === 'opencode' && env === 'docker'))
    return {
      windows: {},
      detail: 'Kontingent in dieser Umgebung nicht verfügbar.',
    };
  if (
    agent === 'claude' &&
    !profiles.find((p) => p.agent === agent && p.id === profile)?.signedIn
  )
    return {
      windows: {},
      detail: 'Bei Claude Code anmelden, um das Kontingent zu sehen.',
    };
  const windows = Object.fromEntries(
    Object.entries(samples[agent]).map(([metric, [used, hours]]) => [
      metric,
      { used, resetAt: new Date(epoch + hours * HOURS).toISOString() },
    ]),
  );
  return { windows, detail: null };
}
