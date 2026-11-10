import type { AgentUpdateStatus } from './types';

/** The native and demo rejection while a run is in progress. */
export const AGENT_UPDATES_BUSY = 'Agenten werden bereits aktualisiert.';

/** How a status row reads: its text, an optional tooltip and its tone. */
export interface AgentUpdateText {
  text: string;
  title?: string;
  /** `fail` draws the failed-state colour; every other state stays plain. */
  tone?: 'fail';
}

/** The German state text of one agent's update row. */
export function agentUpdateText(status: AgentUpdateStatus): AgentUpdateText {
  const { version, from } = status;
  switch (status.state) {
    case 'running':
      return { text: 'wird aktualisiert …' };
    case 'updated':
      return version
        ? {
            text: `aktualisiert auf ${version}`,
            ...(from && { title: `${from} → ${version}` }),
          }
        : { text: 'aktualisiert' };
    case 'current':
      return { text: version ? `aktuell · ${version}` : 'aktuell' };
    case 'inUse':
      return { text: 'in Benutzung – übersprungen' };
    case 'failed':
      return { text: 'fehlgeschlagen', tone: 'fail' };
  }
}

/** True while any agent's update still runs. */
export function updatesRunning(statuses: readonly AgentUpdateStatus[]) {
  return statuses.some((status) => status.state === 'running');
}
