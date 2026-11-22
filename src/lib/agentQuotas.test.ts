import { afterEach, describe, expect, it, vi } from 'vitest';
import { demoSnapshot } from './demoData';
import { AgentQuotas, quotaContexts } from './agentQuotas.svelte';
import { quotaView } from './quotaDisplay';
import type { AgentQuota } from './quotaTypes';

const standard = {
  agent: 'claude',
  profile: 'default',
  env: 'local',
  identity: 'one',
} as const;
const team = { ...standard, profile: 'team', identity: 'two' };

const usage = (used: number, resetAt: string | null = null): AgentQuota => ({
  detail: null,
  windows: { week: { used, resetAt } },
});

function pending() {
  let resolve!: (value: AgentQuota) => void;
  const promise = new Promise<AgentQuota>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe('agent quotas', () => {
  it('switches to the requested profile immediately and ignores late results for the other one', async () => {
    const first = pending();
    const second = pending();
    const get = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const quotas = new AgentQuotas(get);
    quotas.follow([standard]);
    quotas.follow([team]);
    quotas.refresh(true);
    expect(get.mock.calls).toEqual([
      ['claude', 'default', 'local'],
      ['claude', 'team', 'local'],
    ]);
    second.resolve(usage(73));
    await second.promise;
    first.resolve(usage(15));
    await first.promise;
    expect(quotas.read('claude')).toEqual(usage(73));
    quotas.follow([standard]);
    expect(quotas.read('claude')).toEqual(usage(15));
    expect(get).toHaveBeenCalledTimes(2);
  });

  it('isolates late reads when Claude Standard changes config home after adding a profile', async () => {
    const data = demoSnapshot();
    const contexts = () =>
      quotaContexts(data, 'claude', 'default', 'local').filter(
        (c) => c.agent === 'claude',
      );
    const first = pending();
    const get = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(usage(51));
    const quotas = new AgentQuotas(get);
    data.settings.profiles.claude = [];
    quotas.follow(contexts());
    data.settings.profiles.claude = [{ id: 'team', label: 'Team' }];
    quotas.follow(contexts());
    await Promise.resolve();
    first.resolve(usage(32));
    await first.promise;
    expect(quotas.read('claude')).toEqual(usage(51));
  });

  // Until #83 a failed read wiped the last reading; now it keeps the windows and
  // only their own reset removes the percentage.
  it('refreshes at a reported reset and removes the expired percentage when that read fails', async () => {
    vi.useFakeTimers();
    const now = Date.now();
    const value = usage(15, new Date(now + 60_000).toISOString());
    const get = vi
      .fn()
      .mockResolvedValueOnce(value)
      .mockRejectedValue(new Error());
    const quotas = new AgentQuotas(get);
    quotas.follow([standard]);
    await Promise.resolve();
    expect(quotas.read('claude')).toEqual(value);
    vi.setSystemTime(now + 59_999);
    quotas.refresh();
    expect(get).toHaveBeenCalledTimes(1);
    vi.setSystemTime(now + 60_000);
    quotas.refresh();
    await Promise.resolve();
    expect(get).toHaveBeenCalledTimes(2);
    const view = quotaView(quotas.read('claude'), ['week'], quotas.now);
    expect(view.week.used).toBeNull();
    expect(view.hint).toMatch(/Woche · Zeitfenster abgelaufen/);
  });

  it('keeps the last good reading through failed reads and retries on the normal schedule', async () => {
    vi.useFakeTimers();
    const now = Date.now();
    const good = usage(40, new Date(now + 86_400_000).toISOString());
    const failed: AgentQuota = {
      windows: {},
      detail: 'Der Anbieter ist nicht erreichbar.',
    };
    const get = vi
      .fn()
      .mockResolvedValueOnce(good)
      .mockResolvedValueOnce(failed)
      .mockRejectedValueOnce(new Error())
      .mockResolvedValueOnce(usage(55));
    const quotas = new AgentQuotas(get);
    quotas.follow([standard], 5);
    await Promise.resolve();
    vi.setSystemTime(now + 300_000);
    quotas.refresh();
    await Promise.resolve();
    expect(quotas.read('claude')).toEqual({
      windows: good.windows,
      detail: failed.detail,
    });
    vi.setSystemTime(now + 599_999);
    quotas.refresh();
    expect(get).toHaveBeenCalledTimes(2);
    vi.setSystemTime(now + 600_000);
    quotas.refresh();
    await Promise.resolve();
    expect(quotas.read('claude')?.windows).toEqual(good.windows);
    expect(quotas.read('claude')?.detail).toBe(
      'Kontingent konnte nicht geladen werden.',
    );
    vi.setSystemTime(now + 900_000);
    quotas.refresh();
    await Promise.resolve();
    expect(quotas.read('claude')).toEqual(usage(55));
  });

  it('drops the last good reading once reads have failed for a quarter of an hour', async () => {
    vi.useFakeTimers();
    const now = Date.now();
    const good = usage(40, new Date(now + 86_400_000).toISOString());
    const get = vi
      .fn()
      .mockResolvedValueOnce(good)
      .mockRejectedValue(new Error());
    const quotas = new AgentQuotas(get);
    quotas.follow([standard], 5);
    await Promise.resolve();
    for (const minutes of [5, 10]) {
      vi.setSystemTime(now + minutes * 60_000);
      quotas.refresh();
      await Promise.resolve();
      expect(quotas.read('claude')?.windows).toEqual(good.windows);
    }
    vi.setSystemTime(now + 15 * 60_000);
    quotas.refresh();
    await Promise.resolve();
    expect(quotas.read('claude')?.windows).toEqual({});
  });

  it('expires a failed manual refresh before a long polling interval elapses', async () => {
    vi.useFakeTimers();
    const now = Date.now();
    const get = vi
      .fn()
      .mockResolvedValueOnce(usage(25))
      .mockRejectedValue(new Error());
    const quotas = new AgentQuotas(get);
    vi.stubGlobal('window', new EventTarget());
    const stop = quotas.observe();
    quotas.follow([standard], 60);
    await Promise.resolve();
    await vi.advanceTimersByTimeAsync(3 * 60_000);
    quotas.refresh(true);
    await Promise.resolve();
    expect(quotas.read('claude')?.windows).toEqual(usage(25).windows);
    await vi.advanceTimersByTimeAsync(12 * 60_000);
    expect(quotas.read('claude')?.windows).toEqual({});
    expect(quotas.updatedAt('claude')).toBe(now);
    expect(get).toHaveBeenCalledTimes(2);
    stop();
  });

  it('reschedules cached readings from the last attempt when the saved interval changes', async () => {
    vi.useFakeTimers();
    const now = Date.now();
    const get = vi.fn().mockResolvedValue(usage(40));
    const quotas = new AgentQuotas(get);
    quotas.follow([standard], 5);
    await Promise.resolve();
    expect(quotas.updatedAt('claude')).toBe(now);

    vi.setSystemTime(now + 5 * 60_000);
    quotas.follow([standard], 15);
    expect(get).toHaveBeenCalledTimes(1);
    get.mockRejectedValueOnce(new Error());
    quotas.follow([standard], 2);
    await Promise.resolve();
    expect(get).toHaveBeenCalledTimes(2);
    expect(quotas.updatedAt('claude')).toBe(now);

    vi.setSystemTime(now + 7 * 60_000);
    quotas.refresh();
    await Promise.resolve();
    expect(quotas.updatedAt('claude')).toBe(Date.now());
    quotas.follow([]);
    expect(quotas.updatedAt('claude')).toBeUndefined();
  });

  it('shows a first read that fails as unavailable', async () => {
    const get = vi.fn().mockRejectedValue(new Error());
    const quotas = new AgentQuotas(get);
    quotas.follow([standard]);
    await Promise.resolve();
    await Promise.resolve();
    expect(quotas.read('claude')).toEqual({
      windows: {},
      detail: 'Kontingent konnte nicht geladen werden.',
    });
  });

  it('asks nothing for an agent whose read is off and forgets its cached reading', async () => {
    const data = demoSnapshot();
    data.settings.quotaReads = { claude: true, opencode: false, codex: false };
    expect(
      quotaContexts(data, 'claude', 'default', 'local').map((c) => c.agent),
    ).toEqual(['claude']);
    delete data.settings.quotaReads;
    expect(
      quotaContexts(data, 'claude', 'default', 'local').map((c) => c.agent),
    ).toEqual(['opencode']);

    const get = vi.fn().mockResolvedValue(usage(15));
    const quotas = new AgentQuotas(get);
    quotas.follow([standard]);
    await Promise.resolve();
    expect(quotas.read('claude')).toEqual(usage(15));
    quotas.follow([]);
    expect(quotas.read('claude')).toBeUndefined();
    quotas.follow([standard]);
    expect(quotas.read('claude')).toBeUndefined();
    expect(get).toHaveBeenCalledTimes(2);
  });

  it('waits half a minute before the next read when the reported reset is moments away', async () => {
    vi.useFakeTimers();
    const now = Date.now();
    const get = vi
      .fn()
      .mockImplementation(async () =>
        usage(99, new Date(Date.now() + 1_000).toISOString()),
      );
    const quotas = new AgentQuotas(get);
    quotas.follow([standard]);
    await Promise.resolve();
    vi.setSystemTime(now + 29_999);
    quotas.refresh();
    expect(get).toHaveBeenCalledTimes(1);
    vi.setSystemTime(now + 30_000);
    quotas.refresh();
    expect(get).toHaveBeenCalledTimes(2);
  });

  it('updates the display at a reset that falls inside the minimum interval without reading', async () => {
    vi.useFakeTimers();
    const now = Date.now();
    const get = vi
      .fn()
      .mockResolvedValue(usage(100, new Date(now + 10_000).toISOString()));
    const quotas = new AgentQuotas(get);
    vi.stubGlobal('window', new EventTarget());
    const stop = quotas.observe();
    quotas.follow([standard]);
    await Promise.resolve();
    await Promise.resolve();
    vi.advanceTimersByTime(10_000);
    expect(quotas.now).toBe(now + 10_000);
    expect(get).toHaveBeenCalledTimes(1);
    stop();
  });

  it('discards the previous account display while a replacement sign-in or environment is loading', async () => {
    const get = vi.fn().mockResolvedValue(usage(15));
    const quotas = new AgentQuotas(get);
    quotas.follow([standard]);
    await Promise.resolve();
    quotas.follow([{ ...standard, identity: 'new sign-in' }]);
    expect(quotas.read('claude')).toBeUndefined();
    await Promise.resolve();
    quotas.follow([{ ...standard, env: 'docker' }]);
    expect(quotas.read('claude')).toBeUndefined();
    expect(get).toHaveBeenCalledTimes(3);
  });
});
