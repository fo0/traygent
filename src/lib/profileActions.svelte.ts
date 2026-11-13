import type { Bridge } from './bridgeTypes';
import type { AgentId, Snapshot } from './types';

export interface ProfileDeps {
  bridge: Pick<Bridge, 'syncProfiles' | 'profileLogin'>;
  /** Shows a snapshot unless an older one; see `Panel.accept`. */
  accept: (snapshot: Snapshot) => boolean;
}

/**
 * The immediate profile actions of Settings → Agenten (#33): „Jetzt
 * synchronisieren“ and „Anmelden“. They live in the panel, so a closed
 * dialog never drops a result; both reject with the German message, which
 * the dialog shows inline.
 */
export class ProfileActions {
  /** A sync runs; its button waits. */
  syncing = $state(false);
  #deps: ProfileDeps;

  constructor(deps: ProfileDeps) {
    this.#deps = deps;
  }

  /** Syncs every profile now; false when one was already running. */
  async sync() {
    if (this.syncing) return false;
    this.syncing = true;
    try {
      this.#deps.accept(await this.#deps.bridge.syncProfiles());
      return true;
    } finally {
      this.syncing = false;
    }
  }

  /** Opens the terminal that signs `profile` in; the status follows later. */
  login(agent: AgentId, profile: string) {
    return this.#deps.bridge.profileLogin(agent, profile);
  }
}
