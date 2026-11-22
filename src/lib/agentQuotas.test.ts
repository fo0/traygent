import { afterEach, describe, expect, it, vi } from 'vitest';
import { demoSnapshot } from './demoData';
import { AgentQuotas, quotaContexts } from './agentQuotas.svelte';
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

  it('refreshes at a reported reset and removes the last percentage on failure', async () => {
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
    expect(quotas.read('claude')?.windows).toEqual({});
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
