/*
 * One start per press for a „Workspaces“ row. The whole row starts its session
 * (owner request, #17), so the second click of a double-click, a held Enter
 * or Space and a late second click on the same row never start it twice; the
 * guard and the cooldown follow RepoWatch's direct favorite start.
 */

/** How long a row refuses another start after its last start succeeded. */
export const RESTART_COOLDOWN = 1500;

/** The second and later click of a multi-click; a key press reports 0. */
export const repeatClick = (event: { detail: number }) => event.detail > 1;

/** An auto-repeated Enter or Space on the row's start. */
export const repeatKey = (event: { key: string; repeat: boolean }) =>
  event.repeat && (event.key === 'Enter' || event.key === ' ');

/** Lets one start per row through: none while it runs, none in its cooldown. */
export class RestartGate {
  /** Row id → time until which its start stays refused. */
  #until = new Map<string, number>();
  #now: () => number;

  constructor(now: () => number = Date.now) {
    this.#now = now;
  }

  /**
   * Runs `start` for the row unless its start runs or cools down; false when
   * refused. A start that did not happen (refused, failed) leaves no cooldown.
   */
  run(id: string, start: () => Promise<boolean>): boolean {
    if ((this.#until.get(id) ?? 0) > this.#now()) return false;
    this.#until.set(id, Infinity);
    const settle = (started: boolean) => {
      if (started) this.#until.set(id, this.#now() + RESTART_COOLDOWN);
      else this.#until.delete(id);
    };
    void start().then(settle, () => settle(false));
    return true;
  }
}

/**
 * The row a key moves focus to from row `current`, or null for a key the list
 * leaves alone: ↑ and ↓ step by one and stop at the ends, Home and End jump
 * to the first and last row.
 */
export function rowStep(
  key: string,
  current: number,
  count: number,
): number | null {
  if (current < 0 || current >= count) return null;
  switch (key) {
    case 'ArrowDown':
      return Math.min(current + 1, count - 1);
    case 'ArrowUp':
      return Math.max(current - 1, 0);
    case 'Home':
      return 0;
    case 'End':
      return count - 1;
    default:
      return null;
  }
}

/**
 * Svelte action for the „Workspaces“ list: on a row's start, ↑ / ↓, Home and
 * End move focus to another row's start instead of scrolling. Keys with a
 * modifier pass, and „⋯“ keeps its own arrow keys (they open its menu).
 */
export function rowKeys(node: HTMLElement) {
  const onKey = (event: KeyboardEvent) => {
    if (event.defaultPrevented || event.isComposing) return;
    if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey)
      return;
    const target = event.target;
    if (!(target instanceof HTMLElement) || !target.matches('.recent-start'))
      return;
    const starts = [...node.querySelectorAll<HTMLElement>('.recent-start')];
    const next = rowStep(event.key, starts.indexOf(target), starts.length);
    if (next === null) return;
    event.preventDefault();
    starts[next]?.focus();
  };
  node.addEventListener('keydown', onKey);
  return {
    destroy() {
      node.removeEventListener('keydown', onKey);
    },
  };
}
