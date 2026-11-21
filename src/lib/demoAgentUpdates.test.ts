import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  AGENT_UPDATES_BUSY,
  AGENT_UPDATES_DETECTING,
  AGENT_UPDATES_NONE,
} from './agentUpdates';
import { createDemoBridge, demoSnapshot } from './demo';
import { agentUpdatesProblem, agentUpdateSteps } from './demoAgentUpdates';
import type { LaunchRequest, Snapshot } from './types';

afterEach(() => {
  vi.useRealTimers();
});

/** The agents' update states, in row order. */
const states = (snapshot: Snapshot) =>
  snapshot.agentUpdates.map((status) => [status.agent, status.state]);

/** A demo bridge with a step every 300 ms, recording what it pushes. */
async function running() {
  vi.useFakeTimers();
  const bridge = createDemoBridge(0, 600);
  const pushed: Snapshot[] = [];
  await bridge.observe((value) => pushed.push(value));
  const started = bridge.updateAgents();
  await vi.advanceTimersByTimeAsync(0);
  return { bridge, pushed, started: await started };
}

const codexStart = (env: LaunchRequest['env']): LaunchRequest => ({
  agent: 'codex',
  env,
  folder: 'C:\\Users\\demo\\Projekte\\neu',
  options: {
    model: 'gpt-5.5',
    effort: 'high',
    permission: 'ask',
    extraArgs: '',
  },
  session: 'new',
  ports: [],
});

describe('demo update_agents', () => {
  it('starts with the setting off and no run', async () => {
    const snapshot = await createDemoBridge(0, 0).snapshot();
    expect(snapshot.settings.agentUpdates).toBe(false);
    expect(snapshot.agentUpdates).toEqual([]);
    expect(snapshot.agentUpdatesRunning).toBe(false);
  });

  it('takes the ready local agents one after the other, each step pushed', async () => {
    const { pushed, started } = await running();
    // Returns before the first agent turns `running`, as natively.
    expect(started.agentUpdatesRunning).toBe(true);
    expect(started.agentUpdates).toEqual([]);
    expect(pushed).toEqual([]);

    await vi.advanceTimersByTimeAsync(300 * 5);
    expect(pushed.map(states)).toEqual([
      [['claude', 'running']],
      [['claude', 'updated']],
      [
        ['claude', 'updated'],
        ['codex', 'running'],
      ],
      [
        ['claude', 'updated'],
        ['codex', 'current'],
      ],
      [
        ['claude', 'updated'],
        ['codex', 'current'],
      ],
    ]);
    expect(pushed.map((s) => s.agentUpdatesRunning)).toEqual([
      true,
      true,
      true,
      true,
      false,
    ]);
    const revisions = [started, ...pushed].map((s) => s.revision);
    expect(revisions).toEqual([...revisions].sort((a, b) => a - b));
    expect(new Set(revisions).size).toBe(revisions.length);

    const done = pushed.at(-1)!;
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
      { agent: 'codex', state: 'current', from: undefined, version: '0.166.0' },
    ]);
    for (const status of done.agentUpdates)
      expect(Number.isNaN(Date.parse(status.at))).toBe(false);
    // The detection after the run finds the updated CLI.
    const claude = done.agents.find(
      (s) => s.agent === 'claude' && s.env === 'local',
    );
    expect(claude?.version).toBe('2.1.318');
    await vi.advanceTimersByTimeAsync(3000);
    expect(pushed).toHaveLength(5);
  });

  it('rejects a second run while one is in progress, then runs again', async () => {
    const { bridge } = await running();
    const second = bridge.updateAgents();
    const rejected = expect(second).rejects.toThrow(AGENT_UPDATES_BUSY);
    await vi.advanceTimersByTimeAsync(0);
    await rejected;
    await vi.advanceTimersByTimeAsync(300 * 5);
    // Settled: a new run starts again and keeps the last results meanwhile.
    const third = bridge.updateAgents();
    await vi.advanceTimersByTimeAsync(0);
    const again = await third;
    expect(again.agentUpdatesRunning).toBe(true);
    expect(states(again)).toEqual([
      ['claude', 'updated'],
      ['codex', 'current'],
    ]);
  });

  it('refuses while the agents are still being detected', async () => {
    vi.useFakeTimers();
    const bridge = createDemoBridge(0, 600);
    const detecting = bridge.refreshDetection();
    await vi.advanceTimersByTimeAsync(0);
    await detecting;
    const refused = expect(bridge.updateAgents()).rejects.toThrow(
      AGENT_UPDATES_DETECTING,
    );
    await vi.advanceTimersByTimeAsync(0);
    await refused;
    const unchecked = demoSnapshot();
    delete unchecked.checkedAt;
    expect(agentUpdatesProblem(unchecked)).toBe(AGENT_UPDATES_DETECTING);
  });

  it('refuses without a ready local agent', () => {
    const snapshot = demoSnapshot();
    expect(agentUpdatesProblem(snapshot)).toBe('');
    snapshot.agents = snapshot.agents.map((status) =>
      status.env === 'local' ? { ...status, state: 'missing' } : status,
    );
    expect(agentUpdatesProblem(snapshot)).toBe(AGENT_UPDATES_NONE);
  });

  it('refuses a local start of the agent being updated, not a Docker one', async () => {
    const { bridge } = await running();
    await vi.advanceTimersByTimeAsync(300 * 3);
    const local = bridge.launchSession(codexStart('local'));
    const refused = expect(local).rejects.toThrow(
      'Codex wird gerade aktualisiert.',
    );
    const docker = bridge.launchSession(codexStart('docker'));
    await vi.advanceTimersByTimeAsync(0);
    await refused;
    await expect(docker).resolves.toBeTruthy();
    await vi.advanceTimersByTimeAsync(300);
    const after = bridge.launchSession(codexStart('local'));
    await vi.advanceTimersByTimeAsync(0);
    await expect(after).resolves.toBeTruthy();
  });

  it('reports an agent in use as skipped', () => {
    const snapshot = demoSnapshot();
    snapshot.agents = snapshot.agents.map((status) =>
      status.agent === 'opencode' && status.env === 'local'
        ? { ...status, state: 'ready', version: '1.14.0' }
        : status,
    );
    snapshot.agentUpdatesRunning = true;
    for (const step of agentUpdateSteps(snapshot, () => 'at'))
      Object.assign(snapshot, step(snapshot));
    expect(states(snapshot)).toEqual([
      ['claude', 'updated'],
      ['opencode', 'inUse'],
      ['codex', 'current'],
    ]);
    expect(snapshot.agentUpdatesRunning).toBe(false);
  });
});
