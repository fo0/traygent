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

/** From this share used on, a figure and its bar end show the warning state. */
export const QUOTA_WARNING = 70;
/** From this share used on, the critical state. */
export const QUOTA_CRITICAL = 90;

function resetCountdown(milliseconds: number) {
  if (milliseconds >= DAY)
    return plural(Math.round(milliseconds / DAY), 'Tag', 'Tage');
  if (milliseconds >= HOUR) return `${Math.round(milliseconds / HOUR)} Std.`;
  return `${Math.ceil(milliseconds / MINUTE)} Min.`;
}

/** The design token of a metric's colour (`00-tokens.css`): `--quota-spark-week`. */
export const quotaColorToken = (metric: QuotaMetric) =>
  `--quota-${metric.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}`;

/** The metric's colour as a CSS value, for its word, figure and bar (#94). */
export const quotaColor = (metric: QuotaMetric) =>
  `var(${quotaColorToken(metric)})`;

export type QuotaLevel = 'ok' | 'warning' | 'critical';

const level = (used: number): QuotaLevel =>
  used >= QUOTA_CRITICAL
    ? 'critical'
    : used >= QUOTA_WARNING
      ? 'warning'
      : 'ok';

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
      /** The figure the label line shows: the share used, `–` without one. */
      value: '–',
      level: null,
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
    value: `${percent.format(used)} %`,
    level: level(used),
    countdown: reset === null ? '' : resetCountdown(reset - now),
    hint: `${label} · ${percent.format(used)} % verbraucht · ${free} · ${resetHint}`,
  };
}

export type QuotaRow = ReturnType<typeof quotaRow>;

/** One profile's reading, as the tile receives it. */
export interface ProfileReading {
  id: string;
  /** The label line's prefix; `''` for an agent without extra profiles. */
  name: string;
  /** The start profile: its name is emphasized. */
  start: boolean;
  quota: AgentQuota | undefined;
  /** When its last good reading arrived. */
  updatedAt?: number | undefined;
}

/**
 * One label line and its stacked bars: the configured rows, the countdown to
 * the weekly reset, and the profile's part of the description.
 */
function profileView(
  reading: ProfileReading,
  metrics: readonly QuotaMetric[],
  now: number,
) {
  const rows = metrics.map((metric) => quotaRow(reading.quota, metric, now));
  const week = rows.find((row) => row.metric === 'week');
  const lines = [...rows.map((row) => row.hint), reading.quota?.detail]
    .filter(Boolean)
    .map((line) => (reading.name ? `${reading.name} · ${line}` : line));
  return {
    id: reading.id,
    name: reading.name,
    start: reading.start,
    rows,
    countdown: week?.countdown ?? '',
    hint: lines.join('\n'),
  };
}

export type ProfileView = ReturnType<typeof profileView>;

/**
 * The tile's view model (#94): per shown profile one label line with its
 * rows, a weekly summary of the first, and one description for hover and
 * speech. „Stand“ is the oldest good reading among the profiles, so it never
 * claims fresher figures than one of them has. With the read off
 * (`read: false`) the tile shows no figure and no bar: a chosen state, not
 * missing data.
 */
export function quotaView(
  readings: readonly ProfileReading[],
  metrics: readonly QuotaMetric[],
  now: number,
  read = true,
  refreshMinutes?: number,
) {
  const week = quotaRow(readings[0]?.quota, 'week', now);
  if (!read)
    return { week, profiles: [], hint: QUOTA_OFF_HINT, off: true as const };
  const profiles = readings.map((reading) =>
    profileView(reading, metrics, now),
  );
  const good = readings
    .map((reading) => reading.updatedAt)
    .filter((at): at is number => at !== undefined);
  const updatedAt = good.length ? Math.min(...good) : undefined;
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
  const hint = [...profiles.map((profile) => profile.hint), refreshHint]
    .filter(Boolean)
    .join('\n');
  return { week, profiles, hint, updated, refreshHint, off: false as const };
}

export type QuotaView = ReturnType<typeof quotaView>;
