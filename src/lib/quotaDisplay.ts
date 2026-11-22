import { MINUTE } from './clock';
import { dateTime, plural } from './format';
import { QUOTA_LABELS, type AgentQuota, type QuotaMetric } from './quotaTypes';

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

/** Weekly summary and configured rows, with one description for hover and speech. */
export function quotaView(
  quota: AgentQuota | undefined,
  metrics: readonly QuotaMetric[],
  now: number,
) {
  const week = quotaRow(quota, 'week', now);
  const rows = metrics.map((metric) =>
    metric === 'week' ? week : quotaRow(quota, metric, now),
  );
  const hint = [...rows.map((row) => row.hint), quota?.detail]
    .filter(Boolean)
    .join('\n');
  return { week, rows, hint };
}

export type QuotaView = ReturnType<typeof quotaView>;
