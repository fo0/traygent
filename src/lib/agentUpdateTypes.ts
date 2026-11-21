/**
 * The agent-update part of the IPC data model (#16), mirrored by
 * src-tauri/src/models/agent_updates.rs; re-exported from `types.ts`, the
 * contract. Product rules: docs/SPEC.md → Agent updates.
 */
import type { AgentId } from './types';

/** Where one agent's CLI update run stands. */
export type AgentUpdateState =
  'running' | 'updated' | 'current' | 'inUse' | 'failed';

/**
 * Why an agent counted as in use (#48 in BACKLOG.md): a process of its CLI,
 * a node / bun process whose command line cannot be read (it counts for every
 * agent), or a process list that could not be read (fail closed).
 */
export type AgentInUseReason = 'process' | 'unreadableProcess' | 'processList';

/** The latest update run of one agent's local CLI. */
export interface AgentUpdateStatus {
  agent: AgentId;
  state: AgentUpdateState;
  /** CLI version after the run (`updated`, `current`), when known. */
  version?: string;
  /** Version before, only for `updated`. */
  from?: string;
  /** RFC 3339 time this state was reached. */
  at: string;
  /** Why the agent counted as in use; only for `inUse`. */
  reason?: AgentInUseReason;
}
