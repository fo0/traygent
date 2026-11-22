import { MINUTE } from './clock';
import { clockTime, dateTime, plural } from './format';
import {
  QUOTA_LABELS,
  QUOTA_OFF_HINT,
  type AgentQuota,
  type QuotaMetric,
} from './quotaTypes';

const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const percent = new Intl.NumberFormat('de-DE', { maximumFractionDigits: 1 });

function resetCountdown(milliseconds: number) {
  if (milliseconds >= DAY)
    return plural(Math.round(milliseconds / DAY), 'Tag', 'Tage');
  if (milliseconds >= HOUR) return `${Math.round(milliseconds / HOUR)} Std.`;
  return `${Math.ceil(milliseconds / MINUTE)} Min.`;
}

function quotaRow(
  quota: AgentQuota | undefined,
  metric: QuotaMetric,
  now: number,
) {
  const window = quota?.windows[metric];
  const label = QUOTA_LABELS[metric];
  const reset = window?.resetAt == null ? null : Date.parse(window.resetAt);
  if (
    !window ||
    !Number.isFinite(window.used) ||
    window.used < 0 ||
    window.used > 100 ||
    (reset !== null && (!Number.isFinite(reset) || reset <= now))
  ) {
    const reason =
      reset !== null && reset <= now ? 'Zeitfenster abgelaufen' : 'Keine Daten';
    return {
      metric,
      label,
      used: null,
      free: 'Keine Daten',
      countdown: '',
      hint: `${label} · ${reason}`,
    };
  }
  const used = window.used;
  const free = `${percent.format(100 - used)} % frei`;
  const resetHint =
    reset === null ? 'Reset unbekannt' : `Reset am ${dateTime(reset)}`;
  return {
    metric,
    label,
    used,
    free,
    countdown: reset === null ? '' : resetCountdown(reset - now),
    hint: `${label} · ${percent.format(used)} % verbraucht · ${free} · ${resetHint}`,
  };
}

/**
 * Weekly summary and configured rows, with one description for hover and
 * speech. With the read off (`read: false`) the tile shows no figure and no
 * bar: a chosen state, not missing data.
 */
export function quotaView(
  quota: AgentQuota | undefined,
  metrics: readonly QuotaMetric[],
  now: number,
  read = true,
  updatedAt?: number,
  refreshMinutes?: number,
) {
  const week = quotaRow(quota, 'week', now);
  if (!read) return { week, rows: [], hint: QUOTA_OFF_HINT, off: true };
  const rows = metrics.map((metric) =>
    metric === 'week' ? week : quotaRow(quota, metric, now),
  );
  const updated =
    updatedAt === undefined ? 'Stand –' : `Stand ${clockTime(updatedAt)}`;
  const refreshHint = [
    updatedAt === undefined
      ? 'Noch keine Kontingentdaten geladen'
      : `Zuletzt aktualisiert am ${dateTime(updatedAt)}`,
    refreshMinutes === undefined
      ? ''
      : `Bei geöffnetem Panel alle ${refreshMinutes} Min.`,
  ]
    .filter(Boolean)
    .join(' · ');
  const hint = [...rows.map((row) => row.hint), quota?.detail, refreshHint]
    .filter(Boolean)
    .join('\n');
  return { week, rows, hint, updated, refreshHint, off: false };
}

export type QuotaView = ReturnType<typeof quotaView>;
