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

/** A failed sign-in, shown inline as „<Name>: <text>“. */
export interface LoginFailure {
  agent: AgentId;
  profile: string;
  text: string;
  /** Hidden („Debug“ off) and not cancelled: the tooltip suggests „Debug“. */
  hint: boolean;
}

/** The tooltip of a failed hidden sign-in (#41); never visible prose. */
export const DEBUG_HINT = 'Mit Debug erneut versuchen';
/** Natively too: exit code 0 without a sign-in on disk. */
const LOGIN_FAILED = 'Anmeldung fehlgeschlagen.';

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
  /**
   * Profiles whose sign-in is pending, as `agent:id`: hidden until it ended,
   * with „Debug“ until the terminal opened. A second click waits for it.
   */
  signingIn = $state<string[]>([]);
  /**
   * The last failed sign-in; cleared when that profile signs in again, by
   * „Jetzt synchronisieren“ and by adding a profile.
   */
  failure = $state<LoginFailure | null>(null);
  #deps: ProfileDeps;
  /** Sign-ins this panel asked to end: their rejection is no failure. */
  #cancelled = new Set<string>();

  constructor(deps: ProfileDeps) {
    this.#deps = deps;
  }

  /** Whether the sign-in of `profile` is pending. */
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
   * the statuses read anew: only a profile that reads signed in counts. With
   * „Debug“ the terminal opened and the status follows as an event. A
   * failure lands in `failure`, never a rejection.
   */
  async login(agent: AgentId, profile: string): Promise<LoginResult> {
    const key = keyOf(agent, profile);
    if (this.signingIn.includes(key)) return 'failed';
    const hidden = !this.#deps.current()?.settings.debug;
    if (this.failure?.agent === agent && this.failure.profile === profile)
      this.failure = null;
    this.#cancelled.delete(key);
    this.signingIn = [...this.signingIn, key];
    const fail = (text: string, hint: boolean): LoginResult => {
      this.failure = { agent, profile, text, hint };
      return 'failed';
    };
    try {
      const answer = await this.#deps.bridge.profileLogin(agent, profile);
      this.#deps.accept(answer);
      if (!hidden) return 'opened';
      const status = answer.profiles.find(
        (p) => p.agent === agent && p.id === profile,
      );
      return status?.signedIn ? 'signedIn' : fail(LOGIN_FAILED, true);
    } catch (cause) {
      if (this.#cancelled.has(key)) return 'cancelled';
      return fail(errorMessage(cause), hidden);
    } finally {
      this.#cancelled.delete(key);
      this.signingIn = this.signingIn.filter((item) => item !== key);
    }
  }

  /**
   * Ends the pending sign-in of `profile`; the row returns quietly. When
   * Rust had none registered the mark is dropped again, so a later real
   * failure is still reported.
   */
  async cancel(agent: AgentId, profile: string) {
    const key = keyOf(agent, profile);
    if (!this.signingIn.includes(key)) return;
    this.#cancelled.add(key);
    try {
      const found = await this.#deps.bridge.cancelProfileLogin(agent, profile);
      if (!found) this.#cancelled.delete(key);
    } catch (cause) {
      this.#cancelled.delete(key);
      this.failure = { agent, profile, text: errorMessage(cause), hint: false };
    }
  }
}
