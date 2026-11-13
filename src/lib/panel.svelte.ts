import type { NoticeTone } from './StatusBanners.svelte';
import type { Bridge, ObserveSnapshots } from './bridgeTypes';
import { errorMessage } from './format';
import { GithubActions } from './githubActions.svelte';
import {
  LAUNCH_NOTICE,
  Notices,
  PANEL_NOTICE,
  startedNotice,
} from './notices.svelte';
import { ProfileActions } from './profileActions.svelte';
import type {
  AgentId,
  AppSettings,
  LaunchOptions,
  LaunchRequest,
  RecentSession,
  Snapshot,
} from './types';

export interface PanelDeps {
  bridge: Bridge;
  native: boolean;
  observe: ObserveSnapshots;
}

export { LAUNCH_NOTICE } from './notices.svelte';
/** An undo offer stays this long (`agent_docs/ux_rules.md` rule 7). */
export const UNDO_TIMEOUT = 8000;
/** After an install the CLI is detected on demand: the install runs on. */
export const installNote = (demo: boolean) =>
  demo
    ? 'Demo: kein Terminal geöffnet.'
    : 'Installation gestartet – danach aktualisieren (F5).';
const removedKey = (id: string) => `recent:${id}`;

/**
 * Panel data and every mutation. Dialogs keep only their form state, so
 * closing one never drops a running operation's result or busy state.
 */
export class Panel {
  // Snapshots are replaced wholesale, never mutated: no deep proxy needed.
  snapshot = $state.raw<Snapshot | null>(null);
  error = $state('');
  refreshing = $state(false);
  launching = $state(false);

  readonly native: boolean;
  readonly notices = new Notices();
  /** Settings → GitHub and the listing refresh. */
  readonly github: GithubActions;
  /** Settings → Agenten → „Profile“: sync and sign-in. */
  readonly profiles: ProfileActions;
  #bridge: Bridge;
  #observe: ObserveSnapshots;
  /** Bumped by every load, refresh and cleanup; their stale answers are dropped. */
  #epoch = 0;
  /**
   * The revision a refresh answered with: the first finished snapshot newer
   * than it announces the detection. Null when nothing waits.
   */
  #announceAfter: number | null = null;

  constructor({ bridge, native, observe }: PanelDeps) {
    this.#bridge = bridge;
    this.native = native;
    this.#observe = observe;
    this.github = new GithubActions({
      bridge,
      notices: this.notices,
      accept: (value) => this.accept(value),
      current: () => this.snapshot,
    });
    this.profiles = new ProfileActions({
      bridge,
      accept: (value) => this.accept(value),
    });
  }

  /** Native is never labelled demo, not even before its first snapshot. */
  get demo() {
    return this.snapshot ? this.snapshot.demo : !this.native;
  }

  /** True while a detection runs natively or in the demo. */
  get detecting() {
    return !!this.snapshot?.detecting;
  }

  /** The one general notice; operations with results of their own use own keys. */
  showNotice(text: string, tone: NoticeTone = 'subtle') {
    this.notices.show({ key: PANEL_NOTICE, text, tone });
  }

  /**
   * Shows `value` unless it is older than the shown snapshot (an equal
   * revision is shown): answers and pushes may arrive in any order. True when
   * shown.
   */
  accept(value: Snapshot): boolean {
    if (this.snapshot && value.revision < this.snapshot.revision) return false;
    this.snapshot = value;
    this.#announceIfDone();
    return true;
  }

  /** Announces the refresh once a finished snapshot newer than its answer is shown. */
  #announceIfDone() {
    const after = this.#announceAfter;
    const value = this.snapshot;
    if (after === null || !value || value.detecting || value.revision <= after)
      return;
    this.#announceAfter = null;
    this.showNotice(value.demo ? 'Demo aktualisiert.' : 'Aktualisiert.');
  }

  /** Loads the first snapshot and follows pushed ones; returns the cleanup. */
  load(onLoaded?: () => void) {
    const ticket = ++this.#epoch;
    this.#bridge
      .snapshot()
      .then((value) => {
        if (ticket !== this.#epoch) return;
        this.accept(value);
        onLoaded?.();
      })
      .catch((cause) => {
        if (ticket === this.#epoch) this.error = errorMessage(cause);
      });
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    void this.#observe((value) => {
      if (!disposed) this.accept(value);
    }).then((stop) => {
      if (disposed) stop();
      else unsubscribe = stop;
    });
    return () => {
      disposed = true;
      unsubscribe?.();
      this.#epoch++;
      this.refreshing = false;
      this.#announceAfter = null;
    };
  }

  /**
   * Starts a detection. Natively it answers at once with `detecting: true`
   * and pushes the finished snapshot; the notice waits for a finished one
   * newer than that answer, which may also have arrived before it. An answer
   * without `detecting` started nothing (e.g. an aborted run) and announces
   * nothing. Resolves to null when the refresh failed or the panel was
   * cleaned up.
   */
  async refresh(): Promise<Snapshot | null> {
    if (this.refreshing) return null;
    const ticket = ++this.#epoch;
    this.refreshing = true;
    this.error = '';
    this.notices.clear(PANEL_NOTICE);
    this.#announceAfter = null;
    try {
      const value = await this.#bridge.refreshDetection();
      if (ticket !== this.#epoch) return null;
      this.accept(value);
      if (value.detecting) {
        this.#announceAfter = value.revision;
        this.#announceIfDone();
      }
      return value;
    } catch (cause) {
      if (ticket === this.#epoch) this.error = errorMessage(cause);
      return null;
    } finally {
      if (ticket === this.#epoch) this.refreshing = false;
    }
  }

  /** Starts a session; true when the terminal opened, also with a warning. */
  async launch(request: LaunchRequest): Promise<boolean> {
    if (this.launching) return false;
    this.launching = true;
    this.notices.clear(LAUNCH_NOTICE);
    try {
      const outcome = await this.#bridge.launchSession(request);
      this.accept(outcome.snapshot);
      this.notices.show(startedNotice(outcome, request.agent));
      return true;
    } catch (cause) {
      this.notices.show({
        key: LAUNCH_NOTICE,
        text: errorMessage(cause),
        tone: 'danger',
      });
      return false;
    } finally {
      this.launching = false;
    }
  }

  /** Removes a recent session and offers „Rückgängig“ for 8 s. */
  async removeRecent(recent: RecentSession) {
    try {
      this.accept(await this.#bridge.removeRecent(recent.id));
      this.notices.show({
        key: removedKey(recent.id),
        text: 'Aus „Zuletzt“ entfernt.',
        tone: 'subtle',
        timeout: UNDO_TIMEOUT,
        action: {
          label: 'Rückgängig',
          run: () => void this.restoreRecent(recent),
        },
      });
    } catch (cause) {
      this.showNotice(errorMessage(cause), 'danger');
    }
  }

  async restoreRecent(recent: RecentSession) {
    this.notices.clear(removedKey(recent.id));
    try {
      this.accept(await this.#bridge.restoreRecent(recent));
    } catch (cause) {
      this.showNotice(errorMessage(cause), 'danger');
    }
  }

  /**
   * „Bearbeiten …“ of a „Zuletzt“ row; rejects with the German message, so
   * the dialog stays open and shows it.
   */
  async updateRecent(
    recent: RecentSession,
    options: LaunchOptions,
    ports: string[],
    profile?: string,
  ) {
    const { id } = recent;
    this.accept(await this.#bridge.updateRecent(id, options, ports, profile));
  }

  /** Saves the whole settings object; rejects with the German message. */
  async saveSettings(settings: AppSettings) {
    const next = await this.#bridge.saveSettings(settings);
    this.accept(next);
    return next;
  }

  /**
   * Starts an agent CLI update run; the results follow as pushed snapshots.
   * Rejects with the German message (e.g. a run is in progress).
   */
  async updateAgents() {
    this.accept(await this.#bridge.updateAgents());
  }

  /** The folder dialog; null when cancelled or failed (with a notice). */
  async pickFolder(initial: string | null): Promise<string | null> {
    try {
      return await this.#bridge.pickFolder(initial);
    } catch (cause) {
      this.showNotice(errorMessage(cause), 'danger');
      return null;
    }
  }

  /**
   * Opens the install terminal; rejects on failure. No detection follows:
   * the install is still running, the user refreshes (F5) once it is done.
   */
  async installAgent(agent: AgentId) {
    await this.#bridge.installAgent(agent);
  }

  /** The short notice after an install flow closed its dialog. */
  announceInstall() {
    this.showNotice(installNote(this.demo));
  }

  async openFolder(path: string) {
    try {
      await this.#bridge.openFolder(path);
    } catch (cause) {
      this.showNotice(errorMessage(cause), 'danger');
    }
  }

  /** „Pfad kopieren“ of a „Zuletzt“ row: the folder path on the clipboard. */
  async copyFolder(path: string) {
    try {
      await navigator.clipboard.writeText(path);
      this.showNotice('Pfad kopiert.');
    } catch {
      this.showNotice('Pfad konnte nicht kopiert werden.', 'danger');
    }
  }
}
