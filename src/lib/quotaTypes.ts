import type { AgentId, AppSettings } from './types';

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
