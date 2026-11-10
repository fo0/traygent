import { afterEach, describe, expect, it, vi } from 'vitest';
import { AGENT_UPDATES_BUSY } from './agentUpdates';
import { createDemoBridge } from './demo';
import type { Snapshot } from './types';

afterEach(() => {
  vi.useRealTimers();
});

describe('demo update_agents', () => {
  it('starts with the setting off and no run', async () => {
    const snapshot = await createDemoBridge(0, 0).snapshot();
    expect(snapshot.settings.agentUpdates).toBe(false);
    expect(snapshot.agentUpdates).toEqual([]);
  });

  it('runs every catalog agent, then pushes synthetic results', async () => {
    vi.useFakeTimers();
    const bridge = createDemoBridge(0, 600);
    const pushed: Snapshot[] = [];
    await bridge.observe((value) => pushed.push(value));
    const started = bridge.updateAgents();
    await vi.advanceTimersByTimeAsync(0);
    const running = await started;
    expect(running.agentUpdates.map((s) => [s.agent, s.state])).toEqual([
      ['claude', 'running'],
      ['opencode', 'running'],
      ['codex', 'running'],
    ]);
    expect(pushed).toEqual([]);

    await vi.advanceTimersByTimeAsync(600);
    expect(pushed).toHaveLength(1);
    const done = pushed[0]!;
    expect(done.revision).toBeGreaterThan(running.revision);
    expect(
      done.agentUpdates.map(({ agent, state, from, version }) => ({
        agent,
        state,
        from,
        version,
      })),
    ).toEqual([
      {
        agent: 'claude',
        state: 'updated',
        from: '2.1.317',
        version: '2.1.318',
      },
      {
        agent: 'opencode',
        state: 'inUse',
        from: undefined,
        version: undefined,
      },
      { agent: 'codex', state: 'current', from: undefined, version: '0.166.0' },
    ]);
    for (const status of done.agentUpdates)
      expect(Number.isNaN(Date.parse(status.at))).toBe(false);
    // The next detection would find the updated CLI.
    const claude = done.agents.find(
      (s) => s.agent === 'claude' && s.env === 'local',
    );
    expect(claude?.version).toBe('2.1.318');
  });

  it('rejects a second run while one is in progress', async () => {
    vi.useFakeTimers();
    const bridge = createDemoBridge(0, 600);
    const first = bridge.updateAgents();
    await vi.advanceTimersByTimeAsync(0);
    await first;
    const second = bridge.updateAgents();
    const rejected = expect(second).rejects.toThrow(AGENT_UPDATES_BUSY);
    await vi.advanceTimersByTimeAsync(0);
    await rejected;
    await vi.advanceTimersByTimeAsync(600);
    // Settled: a new run starts again.
    const third = bridge.updateAgents();
    await vi.advanceTimersByTimeAsync(0);
    expect((await third).agentUpdates.every((s) => s.state === 'running')).toBe(
      true,
    );
  });
});
