import type { AgentId, AppSettings } from './types';

export const DEFAULT_QUOTA_REFRESH_MINUTES = 15;

export function validQuotaRefresh(minutes: number) {
  return Number.isInteger(minutes) && minutes >= 1 && minutes <= 60;
}

export function quotaRefreshMinutes(
  settings: Pick<AppSettings, 'quotaRefreshMinutes'>,
) {
  const minutes = settings.quotaRefreshMinutes;
  return minutes !== undefined && validQuotaRefresh(minutes)
    ? minutes
    : DEFAULT_QUOTA_REFRESH_MINUTES;
}

export const QUOTA_LABELS = {
  week: 'Woche',
  session: 'Session',
  fable: 'Fable',
  sonnet: 'Sonnet',
  spark: 'Spark',
  sparkWeek: 'Spark-Woche',
  month: 'Monat',
};

export type QuotaMetric = keyof typeof QUOTA_LABELS;

export interface QuotaWindow {
  /** Percentage of the subscription limit already used. */
  used: number;
  resetAt: string | null;
}

export interface AgentQuota {
  windows: Partial<Record<QuotaMetric, QuotaWindow>>;
  detail: string | null;
  /**
   * Only on a read the provider refused as too frequent (HTTP 429, #100):
   * the seconds its `Retry-After` asks to wait, at most a day; `0` without one.
   */
  retryAfterSecs?: number;
}

export const QUOTA_METRICS: Record<AgentId, readonly QuotaMetric[]> = {
  claude: ['week', 'session', 'fable', 'sonnet'],
  opencode: ['week', 'session', 'month'],
  codex: ['week', 'session', 'spark', 'sparkWeek'],
};

/** Preserve the saved order and include the mandatory weekly bar for old settings. */
export function configuredQuotaBars(
  settings: Pick<AppSettings, 'quotaBars'>,
  agent: AgentId,
): QuotaMetric[] {
  const list = settings.quotaBars?.[agent];
  if (!Array.isArray(list)) return ['week'];
  const ordered = [
    ...new Set(list.filter((id) => QUOTA_METRICS[agent].includes(id))),
  ];
  if (!ordered.includes('week')) ordered.unshift('week');
  return ordered;
}

/**
 * Claude and Codex are read with the CLI's subscription sign-in from an
 * undocumented endpoint, so only on request; OpenCode Go uses the user's own
 * API key (ADR-0014). Mirrors `settings::default_quota_read` in Rust.
 */
export const QUOTA_READ_DEFAULTS: Record<AgentId, boolean> = {
  claude: false,
  opencode: true,
  codex: false,
};

/** Whether the panel reads `agent`'s quota; an older snapshot uses the defaults. */
export function quotaReadOn(
  settings: Pick<AppSettings, 'quotaReads'>,
  agent: AgentId,
): boolean {
  const value = settings.quotaReads?.[agent];
  return typeof value === 'boolean' ? value : QUOTA_READ_DEFAULTS[agent];
}

/** At most this many profiles share the quota tile: each one is one more read. */
export const MAX_QUOTA_PROFILES = 3;

/**
 * The profile ids the quota tile shows for `agent` (#94), in the order of
 * `known` (the agent's profiles, the default first): the saved choice
 * (`quotaProfiles`) plus `startProfile`, which is always shown; ids not in
 * `known` are dropped. Past `MAX_QUOTA_PROFILES` the start profile stays and
 * the earliest others fill up. Mirrors `settings::quota_profiles` in Rust.
 */
export function shownQuotaProfiles(
  settings: Pick<AppSettings, 'quotaProfiles'>,
  agent: AgentId,
  startProfile: string,
  known: string[],
): string[] {
  const list = settings.quotaProfiles?.[agent];
  const chosen = new Set(Array.isArray(list) ? list : []);
  chosen.add(startProfile);
  const ordered = known.filter((id) => chosen.has(id));
  if (!known.includes(startProfile)) ordered.unshift(startProfile);
  const others = ordered
    .filter((id) => id !== startProfile)
    .slice(0, MAX_QUOTA_PROFILES - 1);
  return ordered.filter((id) => id === startProfile || others.includes(id));
}

/** The tile's description while the agent's read is off. */
export const QUOTA_OFF_HINT = 'Kontingentabfrage ausgeschaltet';

/** Where the read's sign-in comes from, for the switch's tooltip. */
export function quotaReadTip(agent: AgentId): string {
  return agent === 'opencode'
    ? 'Nutzt den eigenen OpenCode-Go-Schlüssel aus der OpenCode-Anmeldung'
    : 'Nutzt die CLI-Anmeldung gegen eine nicht offiziell dokumentierte Schnittstelle des Anbieters';
}
