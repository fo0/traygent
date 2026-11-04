/*
 * One start per press for a „Zuletzt“ row. The whole row starts its session
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
