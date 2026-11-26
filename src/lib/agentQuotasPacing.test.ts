import { afterEach, describe, expect, it, vi } from 'vitest';
import { AgentQuotas } from './agentQuotas.svelte';
import { clockTime } from './format';
import { quotaView } from './quotaDisplay';
import type { AgentQuota } from './quotaTypes';

/* Paced provider reads and 429 backoff (#100). */
const MINUTE = 60_000;
const account = (profile: string) =>
  ({ agent: 'claude', profile, env: 'local', identity: profile }) as const;
const standard = account('default');

const usage = (used: number, resetAt: string | null = null): AgentQuota => ({
  detail: null,
  windows: { week: { used, resetAt } },
});
const limited = (retryAfterSecs = 0): AgentQuota => ({
  windows: {},
  detail: 'Zu viele Abfragen. Bitte später erneut versuchen.',
  retryAfterSecs,
});

async function flush() {
  for (let i = 0; i < 5; i++) await Promise.resolve();
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe('paced quota reads', () => {
  it('reads three Claude profiles one at a time, at least five seconds apart', async () => {
    vi.useFakeTimers();
    const starts: number[] = [];
    let inFlight = 0;
    let most = 0;
    const get = vi.fn(async () => {
      starts.push(Date.now());
      most = Math.max(most, ++inFlight);
      await new Promise((done) => setTimeout(done, 1_000));
      inFlight--;
      return usage(10);
    });
    const quotas = new AgentQuotas(get, () => 0.5);
    quotas.follow(['default', 'team', 'work'].map(account));
    await vi.advanceTimersByTimeAsync(500);
    // A manual refresh mid-way queues behind the read in flight.
    quotas.refresh(true);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(get).toHaveBeenCalledTimes(3);
    expect(most).toBe(1);
    const gaps = starts.slice(1).map((at, i) => at - starts[i]!);
    // 1 s read, then 5 s gap plus 1 s jitter.
    expect(gaps).toEqual([7_000, 7_000]);
    // Forced again: all three, still paced.
    starts.length = 0;
    quotas.refresh(true);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(get).toHaveBeenCalledTimes(6);
    expect(most).toBe(1);
    expect(starts[1]! - starts[0]!).toBeGreaterThanOrEqual(5_000);
  });

  it('holds queued reads while the panel is hidden', async () => {
    vi.useFakeTimers();
    const page = { hidden: false };
    vi.stubGlobal('document', page);
    const get = vi.fn().mockResolvedValue(usage(10));
    const quotas = new AgentQuotas(get, () => 0);
    quotas.follow([standard, account('team')]);
    await flush();
    page.hidden = true;
    await vi.advanceTimersByTimeAsync(60_000);
    expect(get).toHaveBeenCalledTimes(1);
    page.hidden = false;
    quotas.refresh();
    expect(get).toHaveBeenCalledTimes(2);
  });

  it('backs a limited account off 5, then 10 minutes, past a manual refresh, until a good read', async () => {
    vi.useFakeTimers();
    const t0 = Date.now();
    const get = vi
      .fn()
      .mockResolvedValueOnce(limited())
      .mockResolvedValueOnce(limited())
      .mockResolvedValueOnce(usage(20))
      .mockResolvedValue(limited());
    const quotas = new AgentQuotas(get, () => 0);
    const at = async (ms: number, calls: number) => {
      vi.setSystemTime(ms);
      quotas.refresh(true);
      await flush();
      expect(get).toHaveBeenCalledTimes(calls);
    };
    quotas.follow([standard]);
    await flush();
    expect(get).toHaveBeenCalledTimes(1);
    // The regular 15-minute read comes after the 5-minute backoff: it is named.
    expect(quotas.read('claude')?.detail).toBe(
      `Zu viele Abfragen. Nächster Versuch um ${clockTime(t0 + 15 * MINUTE)}.`,
    );
    await at(t0 + 5 * MINUTE - 1, 1);
    await at(t0 + 5 * MINUTE, 2);
    await at(t0 + 15 * MINUTE - 1, 2);
    await at(t0 + 15 * MINUTE, 3);
    expect(quotas.read('claude')).toEqual(usage(20));
    // The good read ended the streak: the next 429 waits 5 minutes again.
    await at(t0 + 16 * MINUTE, 4);
    await at(t0 + 21 * MINUTE - 1, 4);
    await at(t0 + 21 * MINUTE, 5);
  });

  it('waits a longer Retry-After instead of the backoff, also on the regular schedule', async () => {
    vi.useFakeTimers();
    const t0 = Date.now();
    const get = vi
      .fn()
      .mockResolvedValueOnce(limited(2 * 3_600))
      .mockResolvedValue(usage(5));
    const quotas = new AgentQuotas(get, () => 0);
    quotas.follow([standard], 1);
    await flush();
    vi.setSystemTime(t0 + 120 * MINUTE - 1);
    quotas.refresh();
    quotas.refresh(true);
    expect(get).toHaveBeenCalledTimes(1);
    vi.setSystemTime(t0 + 120 * MINUTE);
    quotas.refresh();
    await flush();
    expect(get).toHaveBeenCalledTimes(2);
    expect(quotas.read('claude')).toEqual(usage(5));
  });

  it('keeps the last figures through 429s until their own reset, not a quarter of an hour', async () => {
    vi.useFakeTimers();
    const t0 = Date.now();
    const reset = new Date(t0 + 2 * 60 * MINUTE).toISOString();
    const good: AgentQuota = {
      detail: null,
      windows: {
        week: { used: 40, resetAt: reset },
        sonnet: { used: 0, resetAt: null },
      },
    };
    const get = vi
      .fn()
      .mockResolvedValueOnce(good)
      .mockResolvedValue(limited());
    const quotas = new AgentQuotas(get, () => 0);
    quotas.follow([standard], 5);
    await flush();
    vi.setSystemTime(t0 + 10 * MINUTE);
    quotas.refresh();
    await flush();
    expect(quotas.read('claude')?.windows).toEqual(good.windows);
    // Past the stale limit only the window without a reset goes.
    vi.setSystemTime(t0 + 20 * MINUTE);
    quotas.refresh();
    await flush();
    expect(get).toHaveBeenCalledTimes(3);
    expect(quotas.read('claude')?.windows).toEqual({ week: good.windows.week });
    expect(quotas.updatedAt('claude')).toBe(t0);
    const reading = {
      id: 'default',
      name: '',
      start: true,
      quota: quotas.read('claude'),
      updatedAt: quotas.updatedAt('claude'),
    };
    const view = quotaView([reading], ['week'], t0 + 20 * MINUTE);
    expect(view.week.used).toBe(40);
    expect(view.hint).toMatch(/Zu viele Abfragen\. Nächster Versuch um/);
    expect(
      quotaView([reading], ['week'], t0 + 2 * 60 * MINUTE).week.used,
    ).toBeNull();
  });

  it('shows a 429 without earlier figures as its detail only', async () => {
    const get = vi.fn().mockResolvedValue(limited());
    const quotas = new AgentQuotas(get, () => 0);
    quotas.follow([standard]);
    await flush();
    expect(quotas.read('claude')?.windows).toEqual({});
    expect(quotas.read('claude')?.retryAfterSecs).toBe(0);
    expect(quotas.updatedAt('claude')).toBeUndefined();
  });
});
