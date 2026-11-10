/*
 * The browser demo's agent CLI update run: synthetic results, never read from
 * the machine. Claude Code updates, OpenCode is in use, Codex is current.
 */
import type { AgentId, AgentUpdateStatus, Snapshot } from './types';

/** Every catalog agent as `running`, in catalog order. */
export function runningUpdates(
  snapshot: Snapshot,
  at = new Date().toISOString(),
): AgentUpdateStatus[] {
  return snapshot.catalog.map((entry) => ({
    agent: entry.id,
    state: 'running',
    at,
  }));
}

/** A plausible next patch version, e.g. `2.1.317` → `2.1.318`. */
const nextPatch = (version: string) =>
  version.replace(/(\d+)$/, (patch) => String(Number(patch) + 1));

/**
 * The finished run: each agent's synthetic result, and the local probe of an
 * updated agent carrying its new version, as the next detection would.
 */
export function finishedUpdates(
  snapshot: Snapshot,
  at = new Date().toISOString(),
): Pick<Snapshot, 'agentUpdates' | 'agents'> {
  const local = (agent: AgentId) =>
    snapshot.agents.find((s) => s.agent === agent && s.env === 'local');
  const agentUpdates = snapshot.catalog.map(({ id }): AgentUpdateStatus => {
    const version = local(id)?.version;
    if (id === 'claude' && version)
      return {
        agent: id,
        state: 'updated',
        from: version,
        version: nextPatch(version),
        at,
      };
    if (id === 'opencode') return { agent: id, state: 'inUse', at };
    return version
      ? { agent: id, state: 'current', version, at }
      : { agent: id, state: 'current', at };
  });
  const agents = snapshot.agents.map((status) => {
    const result = agentUpdates.find(
      (item) => item.agent === status.agent && item.state === 'updated',
    );
    return status.env === 'local' && result?.version
      ? { ...status, version: result.version }
      : status;
  });
  return { agentUpdates, agents };
}
