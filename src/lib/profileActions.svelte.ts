import type { Bridge } from './bridgeTypes';
import { errorMessage } from './format';
import type { AgentId, AppSettings, Snapshot } from './types';

export interface ProfileDeps {
  bridge: Pick<Bridge, 'syncProfiles' | 'profileLogin' | 'cancelProfileLogin'>;
  /** Shows a snapshot unless an older one; see `Panel.accept`. */
  accept: (snapshot: Snapshot) => boolean;
  /** `Panel.saveSettings`: saves and shows the answer. */
  saveSettings: (settings: AppSettings) => Promise<Snapshot>;
  /** The shown snapshot: its saved „Debug“ decides how a sign-in runs. */
  current: () => Snapshot | null;
}

/** How a sign-in ended for the dialog. */
export type LoginResult = 'signedIn' | 'opened' | 'cancelled' | 'failed';

/** A failed sign-in, shown inline until the next profile action. */
export interface LoginFailure {
  agent: AgentId;
  text: string;
  /** Hidden („Debug“ off) and not cancelled: the tooltip suggests „Debug“. */
  hint: boolean;
}

/** The tooltip of a failed hidden sign-in (#41); never visible prose. */
export const DEBUG_HINT = 'Mit Debug erneut versuchen';

const keyOf = (agent: AgentId, profile: string) => `${agent}:${profile}`;

/**
 * The immediate profile actions of Settings → Agenten (#33, #37, #41):
 * „Jetzt synchronisieren“, „Anmelden“ with its cancel and the save of a new
 * profile. They live in the panel, so a closed dialog never drops a result
 * or a running sign-in; each rejects with the German message, which the
 * dialog shows inline.
 */
export class ProfileActions {
  /** A sync runs; its button waits. */
  syncing = $state(false);
  /** Profiles whose hidden sign-in runs, as `agent:id`. */
  signingIn = $state<string[]>([]);
  /** The last failed sign-in; cleared when the next profile action starts. */
  failure = $state<LoginFailure | null>(null);
  #deps: ProfileDeps;
  /** Sign-ins this panel asked to end: their rejection is no failure. */
  #cancelled = new Set<string>();

  constructor(deps: ProfileDeps) {
    this.#deps = deps;
  }

  /** Whether the hidden sign-in of `profile` runs. */
  running(agent: AgentId, profile: string) {
    return this.signingIn.includes(keyOf(agent, profile));
  }

  /** Syncs every profile now; false when one was already running. */
  async sync() {
    if (this.syncing) return false;
    this.syncing = true;
    this.failure = null;
    try {
      this.#deps.accept(await this.#deps.bridge.syncProfiles());
      return true;
    } finally {
      this.syncing = false;
    }
  }

  /**
   * Saves `settings` at once for a new profile (`SettingsDraft.addProfile`
   * sends the baseline plus that profile); resolves to the saved settings.
   */
  save = async (settings: AppSettings) =>
    (await this.#deps.saveSettings(settings)).settings;

  /**
   * Signs `profile` in. Hidden („Debug“ off) it waits for the end and shows
   * the statuses read anew; with „Debug“ the terminal opened and the status
   * follows as an event. A failure lands in `failure`, never a rejection.
   */
  async login(agent: AgentId, profile: string): Promise<LoginResult> {
    const key = keyOf(agent, profile);
    if (this.signingIn.includes(key)) return 'failed';
    const hidden = !this.#deps.current()?.settings.debug;
    this.failure = null;
    this.#cancelled.delete(key);
    if (hidden) this.signingIn = [...this.signingIn, key];
    try {
      this.#deps.accept(await this.#deps.bridge.profileLogin(agent, profile));
      return hidden ? 'signedIn' : 'opened';
    } catch (cause) {
      if (this.#cancelled.has(key)) return 'cancelled';
      this.failure = { agent, text: errorMessage(cause), hint: hidden };
      return 'failed';
    } finally {
      this.#cancelled.delete(key);
      this.signingIn = this.signingIn.filter((item) => item !== key);
    }
  }

  /** Ends the running hidden sign-in of `profile`; the row returns quietly. */
  async cancel(agent: AgentId, profile: string) {
    const key = keyOf(agent, profile);
    if (!this.signingIn.includes(key)) return;
    this.#cancelled.add(key);
    try {
      await this.#deps.bridge.cancelProfileLogin(agent, profile);
    } catch (cause) {
      this.#cancelled.delete(key);
      this.failure = { agent, text: errorMessage(cause), hint: false };
    }
  }
}
