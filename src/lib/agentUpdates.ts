import type { AgentUpdateStatus } from './types';

/** The native and demo rejection while a run is in progress. */
export const AGENT_UPDATES_BUSY = 'Agenten werden bereits aktualisiert.';
/** The rejection while a detection runs or before the first one finished. */
export const AGENT_UPDATES_DETECTING =
  'Die Agenten werden noch gesucht. Bitte gleich noch einmal versuchen.';
/** The rejection without a ready local agent CLI. */
export const AGENT_UPDATES_NONE = 'Kein lokaler Agent installiert.';

/**
 * How a status row reads: its text, an optional tooltip with the same detail
 * for assistive tech, and its tone.
 */
export interface AgentUpdateText {
  text: string;
  title?: string;
  /** The tooltip's detail in words, read as part of the row. */
  detail?: string;
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
            ...(from && {
              title: `${from} → ${version}`,
              detail: `vorher ${from}`,
            }),
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
