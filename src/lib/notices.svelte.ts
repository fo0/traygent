import type { NoticeTone } from './StatusBanners.svelte';
import { agentLabel } from './agents';
import type { AgentId, LaunchOutcome } from './types';

/** Key of the one general panel notice (refresh, settings). */
export const PANEL_NOTICE = 'panel';
/** Key of the start notice: one per panel, a newer start replaces it. */
export const LAUNCH_NOTICE = 'launch';
/** Subtle success notices close on their own after this many milliseconds. */
export const SUCCESS_TIMEOUT = 3000;
const LIMIT = 4;

export interface Notice {
  id: number;
  /**
   * One notice per key: `PANEL_NOTICE` for general messages, an own key per
   * operation, so concurrent results never overwrite each other.
   */
  key: string;
  text: string;
  tone: NoticeTone;
  /** One follow-up action, e.g. „Rückgängig“. */
  action?: { label: string; run: () => void };
  /**
   * Closes after this many milliseconds; subtle notices default to
   * `SUCCESS_TIMEOUT`. Only a notice with an action waits while hovered or
   * focused. Warnings and errors stay until they are closed.
   */
  timeout?: number | undefined;
}

/** Banner notices of the panel, oldest first; successes are bounded. */
export class Notices {
  items = $state.raw<Notice[]>([]);
  #next = 0;

  /** Replaces the notice with the same key. */
  show(notice: Omit<Notice, 'id'>) {
    const entry: Notice = {
      ...notice,
      id: ++this.#next,
      timeout:
        notice.tone === 'subtle'
          ? (notice.timeout ?? SUCCESS_TIMEOUT)
          : undefined,
    };
    const items = [
      ...this.items.filter((item) => item.key !== notice.key),
      entry,
    ];
    // Over the limit, the oldest earlier success goes first; warnings and
    // errors stay until they are closed.
    while (items.length > LIMIT) {
      const oldest = items.findIndex(
        (item) => item.tone === 'subtle' && item !== entry,
      );
      if (oldest < 0) break;
      items.splice(oldest, 1);
    }
    this.items = items;
    return entry.id;
  }
  dismiss(id: number) {
    this.items = this.items.filter((item) => item.id !== id);
  }
  clear(key: string) {
    if (this.items.some((item) => item.key === key))
      this.items = this.items.filter((item) => item.key !== key);
  }
  get(key: string) {
    return this.items.find((item) => item.key === key);
  }
}

/**
 * What a start that opened its terminal says. A warning (e.g. a profile sync
 * that failed) joins it and stays: the session did start.
 */
export function startedNotice(
  outcome: LaunchOutcome,
  agent: AgentId,
): Omit<Notice, 'id'> {
  const label = agentLabel(outcome.snapshot, agent);
  const text = outcome.snapshot.demo
    ? `${label} gestartet (Demo).`
    : `${label} gestartet.`;
  const { warning } = outcome;
  return {
    key: LAUNCH_NOTICE,
    text: warning ? `${text} ${warning}` : text,
    tone: warning ? 'warning' : 'subtle',
  };
}
