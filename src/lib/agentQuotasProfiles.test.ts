import { describe, expect, it, vi } from 'vitest';
import { AgentQuotas } from './agentQuotas.svelte';
import { demoSnapshot } from './demoData';
import { quotaContexts, tileProfiles } from './quotaContexts';
import type { AgentQuota } from './quotaTypes';

/* Several profiles of the selected agent in the quota tile (#94). */
const standard = {
  agent: 'claude',
  profile: 'default',
  env: 'local',
  identity: 'one',
} as const;
const team = { ...standard, profile: 'team', identity: 'two' };

const usage = (used: number): AgentQuota => ({
  detail: null,
  windows: { week: { used, resetAt: null } },
});

function pending() {
  let resolve!: (value: AgentQuota) => void;
  const promise = new Promise<AgentQuota>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe('tile profiles', () => {
  it('names profiles only when the agent has more than „Standard“', () => {
    const data = demoSnapshot();
    data.settings.quotaProfiles = { claude: ['team'] };
    expect(tileProfiles(data, 'claude', 'default')).toEqual([
      { id: 'default', name: 'Standard', start: true },
      { id: 'team', name: 'Team', start: false },
    ]);
    // The start profile is always shown; „Standard“ only when chosen.
    expect(tileProfiles(data, 'claude', 'team')).toEqual([
      { id: 'team', name: 'Team', start: true },
    ]);
    expect(tileProfiles(data, 'claude')).toEqual([
      { id: 'default', name: 'Standard', start: true },
    ]);
    expect(tileProfiles(data, 'codex', 'default')).toEqual([
      { id: 'default', name: '', start: true },
    ]);
    data.settings.profiles.claude = [];
    expect(tileProfiles(data, 'claude', 'default')).toEqual([
      { id: 'default', name: '', start: true },
    ]);
  });

  it('reads every shown profile of the selected agent and the default of the others', () => {
    const data = demoSnapshot();
    data.settings.quotaReads = { claude: true, opencode: true, codex: false };
    data.settings.quotaProfiles = { claude: ['default', 'team'] };
    const contexts = quotaContexts(data, 'claude', 'team', 'local');
    expect(contexts.map((c) => [c.agent, c.profile, c.lead])).toEqual([
      ['claude', 'default', false],
      ['claude', 'team', true],
      ['opencode', 'default', true],
    ]);
    // Another agent selected: Claude reads its saved default alone.
    expect(
      quotaContexts(data, 'opencode', 'default', 'local').map((c) => [
        c.agent,
        c.profile,
      ]),
    ).toEqual([
      ['claude', 'default'],
      ['opencode', 'default'],
    ]);
  });
});

describe('agent quotas per profile', () => {
  it('keeps one entry per shown profile and answers read(agent) for the lead', async () => {
    const get = vi
      .fn()
      .mockImplementation(async (_agent: string, profile: string) =>
        usage(profile === 'team' ? 80 : 30),
      );
    const quotas = new AgentQuotas(get);
    quotas.follow([standard, { ...team, lead: true }]);
    await Promise.resolve();
    expect(get.mock.calls).toEqual([
      ['claude', 'default', 'local'],
      ['claude', 'team', 'local'],
    ]);
    expect(quotas.read('claude', 'default')).toEqual(usage(30));
    expect(quotas.read('claude', 'team')).toEqual(usage(80));
    expect(quotas.read('claude')).toEqual(usage(80));
    expect(quotas.updatedAt('claude', 'team')).toBeTypeOf('number');
    // Without a flag, the agent's first context leads.
    quotas.follow([standard, team]);
    expect(quotas.read('claude')).toEqual(usage(30));
    expect(get).toHaveBeenCalledTimes(2);
  });

  it('lets a late reply update only its own profile, and only while shown', async () => {
    const first = pending();
    const second = pending();
    const get = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const quotas = new AgentQuotas(get);
    quotas.follow([standard, team]);
    quotas.follow([standard]);
    second.resolve(usage(55));
    await second.promise;
    expect(quotas.read('claude', 'team')).toBeUndefined();
    first.resolve(usage(12));
    await first.promise;
    expect(quotas.read('claude', 'default')).toEqual(usage(12));
    expect(quotas.read('claude', 'team')).toBeUndefined();
  });

  it('reads a hidden profile neither while hidden nor when shown again soon', async () => {
    const get = vi.fn().mockResolvedValue(usage(40));
    const quotas = new AgentQuotas(get);
    quotas.follow([standard, team]);
    await Promise.resolve();
    expect(get).toHaveBeenCalledTimes(2);
    quotas.follow([standard]);
    quotas.refresh();
    expect(quotas.read('claude', 'team')).toBeUndefined();
    // Back within the minimum interval (ADR-0014): the kept reading, no request.
    quotas.follow([standard, team]);
    expect(quotas.read('claude', 'team')).toEqual(usage(40));
    expect(get).toHaveBeenCalledTimes(2);
  });

  it('forgets every profile of an agent whose read is switched off', async () => {
    const get = vi.fn().mockResolvedValue(usage(15));
    const quotas = new AgentQuotas(get);
    quotas.follow([standard, team]);
    await Promise.resolve();
    expect(quotas.read('claude', 'team')).toEqual(usage(15));
    quotas.follow([]);
    quotas.follow([standard, team]);
    expect(quotas.read('claude', 'default')).toBeUndefined();
    expect(quotas.read('claude', 'team')).toBeUndefined();
    expect(get).toHaveBeenCalledTimes(4);
  });
});
