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

/** How long typed letters keep adding to one type-ahead search. */
export const TYPEAHEAD_RESET = 700;

/**
 * The row whose name starts with the typed `query` (case-insensitive), or
 * null: the search runs from row `current` itself, so a longer prefix keeps a
 * row that still matches, and wraps at the end. One letter typed again and
 * again („t“, „tt“, …) steps through the rows starting with it instead.
 */
export function typeaheadRow(
  names: readonly string[],
  current: number,
  query: string,
): number | null {
  const typed = query.toLocaleLowerCase('de-DE');
  if (!typed || !names.length) return null;
  const repeated = [...typed].every((char) => char === typed[0]);
  const prefix = repeated ? typed[0]! : typed;
  const from = Math.max(current, 0) + (repeated ? 1 : 0);
  for (let step = 0; step < names.length; step++) {
    const index = (from + step) % names.length;
    if (names[index]!.toLocaleLowerCase('de-DE').startsWith(prefix))
      return index;
  }
  return null;
}

/**
 * Svelte action for the „Workspaces“ list: on a row's start, ↑ / ↓, Home and
 * End move focus to another row's start instead of scrolling, and typed
 * letters move it to the next row whose folder name starts with them (a
 * Space in the middle of such a search belongs to it and never starts the
 * row). Keys with a modifier pass, and „⋯“ keeps its own keys (its arrow
 * keys open its menu).
 */
export function rowKeys(node: HTMLElement) {
  let typed = '';
  let typedAt = -Infinity;
  const onKey = (event: KeyboardEvent) => {
    if (event.defaultPrevented || event.isComposing) return;
    // Shift only types capitals into the search; Shift with ↑ / ↓ still passes.
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const target = event.target;
    if (!(target instanceof HTMLElement) || !target.matches('.recent-start'))
      return;
    const starts = [...node.querySelectorAll<HTMLElement>('.recent-start')];
    const current = starts.indexOf(target);
    const now = Date.now();
    const searching = now - typedAt < TYPEAHEAD_RESET;
    if (event.key.length === 1 && (event.key !== ' ' || (searching && typed))) {
      event.preventDefault();
      typed = (searching ? typed : '') + event.key;
      typedAt = now;
      const names = starts.map(
        (start) =>
          start.closest('li')?.querySelector('.recent-name')?.textContent ?? '',
      );
      const next = typeaheadRow(names, current, typed);
      if (next !== null) starts[next]?.focus();
      return;
    }
    if (event.shiftKey) return;
    const next = rowStep(event.key, current, starts.length);
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
