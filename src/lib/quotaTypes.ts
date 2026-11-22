import type { AgentId, AppSettings } from './types';

export const DEFAULT_QUOTA_REFRESH_MINUTES = 5;

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

/** The tile's description while the agent's read is off. */
export const QUOTA_OFF_HINT = 'Kontingentabfrage ausgeschaltet';

/** Where the read's sign-in comes from, for the switch's tooltip. */
export function quotaReadTip(agent: AgentId): string {
  return agent === 'opencode'
    ? 'Nutzt den eigenen OpenCode-Go-Schlüssel aus der OpenCode-Anmeldung'
    : 'Nutzt die CLI-Anmeldung gegen eine nicht offiziell dokumentierte Schnittstelle des Anbieters';
}
