/*
 * The browser demo's agent CLI update run: synthetic results, never read from
 * the machine. Like natively, a run takes the ready local CLIs one after the
 * other: Claude Code updates, Codex is current; OpenCode, not installed
 * locally in the demo, would be in use.
 */
import { agentLabel, agentStatus } from './agents';
import {
  AGENT_UPDATES_BUSY,
  AGENT_UPDATES_DETECTING,
  AGENT_UPDATES_NONE,
} from './agentUpdates';
import type {
  AgentId,
  AgentUpdateStatus,
  LaunchRequest,
  Snapshot,
} from './types';

/** One step of a run: the patch it applies to the snapshot of that moment. */
export type AgentUpdateStep = (current: Snapshot) => Partial<Snapshot>;

/** The agents a run takes: the ready local CLIs, in catalog order. */
const targets = (snapshot: Snapshot): AgentId[] =>
  snapshot.catalog
    .map((entry) => entry.id)
    .filter((id) => agentStatus(snapshot, id, 'local')?.state === 'ready');

/** Why `update_agents` refuses now, as natively; `''` when a run may start. */
export function agentUpdatesProblem(snapshot: Snapshot): string {
  if (snapshot.agentUpdatesRunning) return AGENT_UPDATES_BUSY;
  if (snapshot.detecting || !snapshot.checkedAt) return AGENT_UPDATES_DETECTING;
  if (!targets(snapshot).length) return AGENT_UPDATES_NONE;
  return '';
}

/** Why a start must wait: a local start of an agent whose update runs. */
export function launchUpdateProblem(
  snapshot: Snapshot,
  request: Pick<LaunchRequest, 'agent' | 'env'>,
): string {
  const running = snapshot.agentUpdates.some(
    (status) => status.agent === request.agent && status.state === 'running',
  );
  return request.env === 'local' && running
    ? `${agentLabel(snapshot, request.agent)} wird gerade aktualisiert.`
    : '';
}

/** A plausible next patch version, e.g. `2.1.317` → `2.1.318`. */
const nextPatch = (version: string) =>
  version.replace(/(\d+)$/, (patch) => String(Number(patch) + 1));

/** The synthetic result of one agent's update. */
function result(
  agent: AgentId,
  version: string | undefined,
  at: string,
): AgentUpdateStatus {
  if (agent === 'claude' && version)
    return {
      agent,
      state: 'updated',
      from: version,
      version: nextPatch(version),
      at,
    };
  if (agent === 'opencode') return { agent, state: 'inUse', at };
  return version
    ? { agent, state: 'current', version, at }
    : { agent, state: 'current', at };
}

/** The statuses with `status` in place of its agent's, in catalog order. */
function withStatus(
  snapshot: Snapshot,
  status: AgentUpdateStatus,
): AgentUpdateStatus[] {
  return snapshot.catalog.flatMap(({ id }) => {
    const item =
      id === status.agent
        ? status
        : snapshot.agentUpdates.find((entry) => entry.agent === id);
    return item ? [item] : [];
  });
}

/**
 * The steps of a run started on `snapshot`: per ready local agent `running`,
 * then its result; last the run ends and the local probe of an updated agent
 * carries its new version, as the detection after a native run would.
 */
export function agentUpdateSteps(
  snapshot: Snapshot,
  now = () => new Date().toISOString(),
): AgentUpdateStep[] {
  const steps = targets(snapshot).flatMap((agent): AgentUpdateStep[] => {
    const version = agentStatus(snapshot, agent, 'local')?.version;
    return [
      (current) => ({
        agentUpdates: withStatus(current, {
          agent,
          state: 'running',
          at: now(),
        }),
      }),
      (current) => ({
        agentUpdates: withStatus(current, result(agent, version, now())),
      }),
    ];
  });
  const finish: AgentUpdateStep = (current) => ({
    agentUpdatesRunning: false,
    agents: current.agents.map((status) => {
      const updated = current.agentUpdates.find(
        (item) => item.agent === status.agent && item.state === 'updated',
      );
      return status.env === 'local' && updated?.version
        ? { ...status, version: updated.version }
        : status;
    }),
  });
  return [...steps, finish];
}

/** Runs each step `every` ms after the one before; `apply` commits it. */
export function scheduleSteps(
  steps: readonly AgentUpdateStep[],
  apply: (step: AgentUpdateStep) => void,
  every: number,
) {
  const [step, ...rest] = steps;
  if (!step) return;
  setTimeout(() => {
    apply(step);
    scheduleSteps(rest, apply, every);
  }, every);
}
