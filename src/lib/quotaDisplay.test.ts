import { describe, expect, it } from 'vitest';
import { clockTime } from './format';
import {
  quotaColorToken,
  quotaView,
  type ProfileReading,
} from './quotaDisplay';
import { QUOTA_OFF_HINT, type AgentQuota } from './quotaTypes';

const now = Date.parse('2026-11-22T12:00:00Z');
const inDays = (days: number) =>
  new Date(now + days * 86_400_000).toISOString();

const quota = (
  windows: AgentQuota['windows'],
  detail: string | null = null,
): AgentQuota => ({ windows, detail });

const reading = (
  id: string,
  value: AgentQuota | undefined,
  extra: Partial<ProfileReading> = {},
): ProfileReading => ({
  id,
  name: '',
  start: true,
  quota: value,
  ...extra,
});

describe('quota view', () => {
  it('shows the used share in German format and its state from 70 and 90 % on', () => {
    const view = quotaView(
      [
        reading(
          'default',
          quota({
            week: { used: 69.9, resetAt: inDays(3) },
            session: { used: 70, resetAt: null },
            fable: { used: 89.95, resetAt: null },
            sonnet: { used: 90, resetAt: null },
          }),
        ),
      ],
      ['week', 'session', 'fable', 'sonnet'],
      now,
    );
    const rows = view.profiles[0]!.rows;
    expect(rows.map((row) => [row.value, row.level])).toEqual([
      ['69,9 %', 'ok'],
      ['70 %', 'warning'],
      ['90 %', 'warning'],
      ['90 %', 'critical'],
    ]);
    expect(view.profiles[0]!.countdown).toBe('3 Tage');
    expect(view.hint).toMatch(
      /^Woche · 69,9 % verbraucht · 30,1 % frei · Reset am /,
    );
  });

  it('shows no figure, no state and no countdown without a valid window', () => {
    const view = quotaView(
      [
        reading(
          'default',
          quota(
            { week: { used: 140, resetAt: inDays(1) } },
            'Bei Claude Code anmelden, um das Kontingent zu sehen.',
          ),
        ),
      ],
      ['week', 'fable'],
      now,
    );
    const [profile] = view.profiles;
    expect(
      profile!.rows.map((row) => [row.value, row.level, row.used]),
    ).toEqual([
      ['–', null, null],
      ['–', null, null],
    ]);
    expect(profile!.countdown).toBe('');
    expect(profile!.hint).toBe(
      'Woche · Keine Daten\nFable · Keine Daten\nBei Claude Code anmelden, um das Kontingent zu sehen.',
    );
  });

  it('gives each shown profile its line and names it in the description', () => {
    const view = quotaView(
      [
        reading('default', quota({ week: { used: 64, resetAt: inDays(3) } }), {
          name: 'Standard',
          updatedAt: now - 60_000,
        }),
        reading('team', quota({ week: { used: 7, resetAt: inDays(0.25) } }), {
          name: 'Team',
          start: false,
          updatedAt: now - 5 * 60_000,
        }),
        reading('privat', undefined, { name: 'Privat', start: false }),
      ],
      ['week'],
      now,
      true,
      15,
    );
    expect(
      view.profiles.map((p) => [p.id, p.name, p.start, p.countdown]),
    ).toEqual([
      ['default', 'Standard', true, '3 Tage'],
      ['team', 'Team', false, '6 Std.'],
      ['privat', 'Privat', false, ''],
    ]);
    expect(view.hint.split('\n').slice(0, 3)).toEqual([
      expect.stringMatching(/^Standard · Woche · 64 % verbraucht · 36 % frei/),
      expect.stringMatching(/^Team · Woche · 7 % verbraucht · 93 % frei/),
      'Privat · Woche · Keine Daten',
    ]);
    // „Stand“ is the oldest good reading, never the freshest.
    expect(view.off).toBe(false);
    if (!view.off) {
      expect(view.refreshHint).toMatch(/alle 15 Min\.$/);
      expect(view.updated).toBe(`Stand ${clockTime(now - 5 * 60_000)}`);
    }
  });

  it('claims no time without any good reading', () => {
    const view = quotaView([reading('default', undefined)], ['week'], now);
    expect(view.off || view.updated).toBe('Stand –');
  });

  it('shows no profile and no figure while the read is off', () => {
    const view = quotaView(
      [reading('default', quota({ week: { used: 10, resetAt: null } }))],
      ['week'],
      now,
      false,
    );
    expect(view).toMatchObject({
      profiles: [],
      hint: QUOTA_OFF_HINT,
      off: true,
    });
  });

  it('names each metric colour token after the metric', () => {
    expect(quotaColorToken('week')).toBe('--quota-week');
    expect(quotaColorToken('sparkWeek')).toBe('--quota-spark-week');
  });
});
