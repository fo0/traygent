export const FALLBACK_ERROR =
  'Der Vorgang konnte nicht abgeschlossen werden. Versuche es erneut.';

/** User-facing text for a rejection: an Error's message, an IPC error string, or a fallback. */
export function errorMessage(cause: unknown, fallback = FALLBACK_ERROR) {
  return cause instanceof Error
    ? cause.message
    : typeof cause === 'string'
      ? cause
      : fallback;
}

/**
 * User-facing text for a rejected native command. Tauri rejects with the Rust
 * error string (German, written for the user); an `Error` can only come from
 * JavaScript internals and would surface English internals, so it and any
 * other value get the fallback. The demo bridge throws German `Error`s and
 * keeps `errorMessage`.
 */
export function ipcErrorMessage(cause: unknown, fallback = FALLBACK_ERROR) {
  return typeof cause === 'string' && cause.trim() ? cause : fallback;
}

/** A count with its German noun form, e.g. `1 Agent` or `3 Agenten`. */
export function plural(count: number, one: string, many: string) {
  return `${count} ${count === 1 ? one : many}`;
}

/** Local wall-clock time as `HH:MM` (de-DE); defaults to now. */
export function clockTime(value: string | number | Date = new Date()) {
  return new Date(value).toLocaleTimeString('de-DE', {
    hour: '2-digit',
    minute: '2-digit',
  });
}

/**
 * A coming moment as „um 09:42“, or „am 28.11. um 09:42“ when it falls on
 * another calendar day than `now` (de-DE), so a wait of a day or more never
 * reads like a time later today.
 */
export function atTime(
  value: string | number | Date,
  now: number = Date.now(),
) {
  const time = new Date(value);
  const sameDay = time.toDateString() === new Date(now).toDateString();
  const day = time.toLocaleDateString('de-DE', {
    day: '2-digit',
    month: '2-digit',
  });
  return `${sameDay ? '' : `am ${day} `}um ${clockTime(time)}`;
}

/** Local date and time as `01.11.2026, 09:42` (de-DE). */
export function dateTime(value: string | number | Date) {
  return new Date(value).toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const dayStart = (time: number) => new Date(time).setHours(0, 0, 0, 0);

/**
 * German relative time: „gerade eben“, „vor 3 Min.“, „vor 2 Std.“ (under a
 * day, or earlier the same calendar day: a 25-hour DST day reaches „vor 24
 * Std.“), „gestern“ (the previous calendar day), „vor 2 Tagen“ up to „vor 6
 * Tagen“ (calendar days), then the date. A time ahead of `now` (clock skew,
 * a stale tick) reads „gerade eben“.
 */
export function relativeTime(
  value: string | number | Date,
  now: number = Date.now(),
) {
  const time = new Date(value).getTime();
  if (Number.isNaN(time)) return '';
  const elapsed = now - time;
  if (elapsed < MINUTE) return 'gerade eben';
  if (elapsed < HOUR) return `vor ${Math.floor(elapsed / MINUTE)} Min.`;
  if (elapsed < DAY || dayStart(time) === dayStart(now))
    return `vor ${Math.floor(elapsed / HOUR)} Std.`;
  // Calendar days between both midnights; rounding absorbs a DST hour.
  const days = Math.round((dayStart(now) - dayStart(time)) / DAY);
  if (days <= 1) return 'gestern';
  if (days <= 6) return `vor ${days} Tagen`;
  return new Date(time).toLocaleDateString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
  });
}
