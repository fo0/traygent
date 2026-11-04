import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest';
import { get } from 'svelte/store';
import { minuteClock } from './clock';
import {
  clockTime,
  dateTime,
  errorMessage,
  FALLBACK_ERROR,
  ipcErrorMessage,
  plural,
  relativeTime,
} from './format';

describe('errorMessage', () => {
  it('prefers an Error message, then an IPC string', () => {
    expect(errorMessage(new Error('Kaputt.'))).toBe('Kaputt.');
    expect(errorMessage('IPC-Fehler.')).toBe('IPC-Fehler.');
  });
  it('falls back for anything else', () => {
    expect(errorMessage({ code: 1 })).toBe(FALLBACK_ERROR);
    expect(errorMessage(undefined, 'Eigener Text.')).toBe('Eigener Text.');
  });
});

describe('ipcErrorMessage', () => {
  it('shows the native error string', () => {
    expect(ipcErrorMessage('Token wurde abgelehnt.')).toBe(
      'Token wurde abgelehnt.',
    );
  });
  it('never shows JavaScript internals, blank strings or other values', () => {
    expect(
      ipcErrorMessage(new TypeError('x is undefined'), 'Eigener Text.'),
    ).toBe('Eigener Text.');
    expect(ipcErrorMessage('  ', 'Eigener Text.')).toBe('Eigener Text.');
    expect(ipcErrorMessage({ code: 1 })).toBe(FALLBACK_ERROR);
    expect(ipcErrorMessage(undefined)).toBe(FALLBACK_ERROR);
  });
});

describe('plural', () => {
  it('uses the singular only for exactly one', () => {
    expect(plural(1, 'Agent', 'Agenten')).toBe('1 Agent');
    expect(plural(1, 'Ordner', 'Ordner')).toBe('1 Ordner');
  });
  it('uses the plural for zero and several', () => {
    expect(plural(0, 'Agent', 'Agenten')).toBe('0 Agenten');
    expect(plural(2, 'Agent', 'Agenten')).toBe('2 Agenten');
    expect(plural(12, 'Session', 'Sessions')).toBe('12 Sessions');
  });
});

describe('clockTime', () => {
  it('formats local time as two-digit hours and minutes', () => {
    expect(clockTime(new Date(2026, 4, 1, 9, 5))).toBe('09:05');
    expect(clockTime(new Date(2026, 4, 1, 21, 40).toISOString())).toBe('21:40');
  });
  it('defaults to the current time', () => {
    expect(clockTime()).toMatch(/^\d{2}:\d{2}$/);
  });
});

describe('relative times', () => {
  // Mid-June keeps every time zone clear of a daylight-saving switch.
  const noon = new Date(2026, 5, 17, 12, 0);
  const at = (day: number, hour: number, minute = 0) =>
    new Date(2026, 5, day, hour, minute).toISOString();
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(noon);
  });
  afterEach(() => vi.useRealTimers());

  it('counts minutes, then hours, as the clock advances', () => {
    const started = at(17, 12);
    expect(relativeTime(started)).toBe('gerade eben');
    vi.advanceTimersByTime(59_000);
    expect(relativeTime(started)).toBe('gerade eben');
    vi.advanceTimersByTime(1_000);
    expect(relativeTime(started)).toBe('vor 1 Min.');
    vi.advanceTimersByTime(58 * 60_000);
    expect(relativeTime(started)).toBe('vor 59 Min.');
    vi.advanceTimersByTime(60_000);
    expect(relativeTime(started)).toBe('vor 1 Std.');
    vi.advanceTimersByTime(22 * 3_600_000 + 59 * 60_000);
    expect(relativeTime(started)).toBe('vor 23 Std.');
    vi.advanceTimersByTime(60_000);
    expect(relativeTime(started)).toBe('gestern');
  });
  it('says „gestern“ for the previous calendar day, then days, then the date', () => {
    expect(relativeTime(at(16, 13))).toBe('vor 23 Std.');
    expect(relativeTime(at(16, 11))).toBe('gestern');
    expect(relativeTime(at(16, 0, 5))).toBe('gestern');
    expect(relativeTime(at(15, 23, 59))).toBe('vor 2 Tagen');
    expect(relativeTime(at(14, 12))).toBe('vor 3 Tagen');
    expect(relativeTime(at(11, 0, 1))).toBe('vor 6 Tagen');
    expect(relativeTime(at(10, 23, 59))).toBe('10.06.2026');
    vi.setSystemTime(new Date(2026, 5, 17, 0, 30));
    expect(relativeTime(at(16, 0, 10))).toBe('gestern');
    expect(relativeTime(at(15, 23))).toBe('vor 2 Tagen');
    expect(relativeTime(at(11, 0))).toBe('vor 6 Tagen');
    expect(relativeTime(at(10, 23, 59))).toBe('10.06.2026');
    // Calendar days, not 24-hour spans: late yesterday and early today.
    vi.setSystemTime(new Date(2026, 5, 17, 23, 59));
    expect(relativeTime(at(15, 0, 1))).toBe('vor 2 Tagen');
    expect(relativeTime(at(10, 23, 59))).toBe('10.06.2026');
  });
  it('reads future times as „gerade eben“ and invalid ones as empty', () => {
    expect(relativeTime(at(17, 12, 5))).toBe('gerade eben');
    expect(relativeTime('kein Datum')).toBe('');
  });
  it('formats the absolute local date and time', () => {
    expect(dateTime(new Date(2026, 5, 17, 9, 5))).toBe('17.06.2026, 09:05');
  });

  it('ticks one shared minute clock that stops with its last subscriber', () => {
    const seen: number[] = [];
    const first = minuteClock.subscribe((value) => seen.push(value));
    const second = minuteClock.subscribe(() => {});
    expect(vi.getTimerCount()).toBe(1);
    expect(seen.at(-1)).toBe(noon.getTime());
    vi.advanceTimersByTime(60_000);
    expect(seen.at(-1)).toBe(noon.getTime() + 60_000);
    first();
    expect(vi.getTimerCount()).toBe(1);
    second();
    expect(vi.getTimerCount()).toBe(0);
  });
  it('lets a view that joins a running clock catch up at once', async () => {
    const seen: number[] = [];
    const overview = minuteClock.subscribe((value) => seen.push(value));
    vi.setSystemTime(noon.getTime() + 45_000);
    const page = minuteClock.subscribe(() => {});
    // No tick is due yet; the new reader still gets the current time.
    expect(vi.getTimerCount()).toBe(1);
    await Promise.resolve();
    expect(seen.at(-1)).toBe(noon.getTime() + 45_000);
    // The next tick counts from the catch-up, still on one shared timer.
    expect(vi.getTimerCount()).toBe(1);
    vi.advanceTimersByTime(59_000);
    expect(seen.at(-1)).toBe(noon.getTime() + 45_000);
    vi.advanceTimersByTime(1_000);
    expect(seen.at(-1)).toBe(noon.getTime() + 105_000);
    overview();
    page();
  });
  it('moves relative times with each tick', () => {
    const started = at(17, 12);
    const stop = minuteClock.subscribe(() => {});
    expect(relativeTime(started, get(minuteClock))).toBe('gerade eben');
    vi.advanceTimersByTime(3 * 60_000);
    expect(relativeTime(started, get(minuteClock))).toBe('vor 3 Min.');
    stop();
  });
});

describe('relative times across a daylight-saving switch', () => {
  // Node re-reads TZ on assignment; restore it for the rest of the file.
  const zone = process.env.TZ;
  beforeAll(() => {
    process.env.TZ = 'Europe/Berlin';
  });
  afterAll(() => {
    if (zone === undefined) delete process.env.TZ;
    else process.env.TZ = zone;
  });

  it('keeps hours for earlier the same day on the 25-hour day', () => {
    // 25.10.2026 23:30 CET; 00:10 that day was still CEST (24 h 20 min ago).
    const now = Date.parse('2026-10-25T22:30:00Z');
    expect(new Date(now).getHours()).toBe(23);
    expect(relativeTime('2026-10-24T22:10:00Z', now)).toBe('vor 24 Std.');
    // 24.10. 23:50 CEST is the previous calendar day.
    expect(relativeTime('2026-10-24T21:50:00Z', now)).toBe('gestern');
  });
  it('keeps „gestern“ and hours right on the 23-hour day', () => {
    // 29.03.2026 23:30 CEST; 00:10 that day was CET (22 h 20 min ago).
    const now = Date.parse('2026-03-29T21:30:00Z');
    expect(relativeTime('2026-03-28T23:10:00Z', now)).toBe('vor 22 Std.');
    // 28.03. 00:10 CET: 46 h 20 min ago, still the previous calendar day.
    expect(relativeTime('2026-03-27T23:10:00Z', now)).toBe('gestern');
    expect(relativeTime('2026-03-27T22:50:00Z', now)).toBe('vor 2 Tagen');
  });
  it('counts calendar days across both switches', () => {
    // 30.03.2026 00:30 CEST; 24.03. 00:10 CET is six calendar days back,
    // although the 23-hour day makes it 5 days 23 h 20 min.
    const spring = Date.parse('2026-03-29T22:30:00Z');
    expect(relativeTime('2026-03-23T23:10:00Z', spring)).toBe('vor 6 Tagen');
    expect(relativeTime('2026-03-23T22:50:00Z', spring)).toBe('23.03.2026');
    // 31.10.2026 23:30 CET; 25.10. 00:10 CEST is six calendar days back,
    // although the 25-hour day makes it 7 days 20 min.
    const autumn = Date.parse('2026-10-31T22:30:00Z');
    expect(relativeTime('2026-10-24T22:10:00Z', autumn)).toBe('vor 6 Tagen');
    expect(relativeTime('2026-10-24T21:50:00Z', autumn)).toBe('24.10.2026');
  });
});
