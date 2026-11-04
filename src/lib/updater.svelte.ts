import { clockTime, errorMessage } from './format';
import { call, channel } from './ipc';
import type { DownloadProgress, UpdateInfo } from './types';
import type { UpdatePhase } from './updates';

const BUSY_PHASES: UpdatePhase[] = [
  'checking',
  'downloading',
  'verifying',
  'installing',
];
const INSTALL_PHASES: UpdatePhase[] = [
  'downloading',
  'verifying',
  'installing',
];

/**
 * In-app update flow; only the native shell reaches the updater commands.
 * State is read-only from outside: only `check` and `install` change it.
 */
export class Updater {
  #info = $state<UpdateInfo | null>(null);
  #phase = $state<UpdatePhase>('idle');
  #progress = $state<DownloadProgress>({
    downloaded: 0,
    total: null,
    stage: 'downloading',
  });
  #message = $state('');
  #checkedAt = $state('');
  #busy = $derived(BUSY_PHASES.includes(this.#phase));
  #enabled: boolean;

  constructor(enabled: boolean) {
    this.#enabled = enabled;
  }

  get info() {
    return this.#info;
  }
  get phase() {
    return this.#phase;
  }
  get progress() {
    return this.#progress;
  }
  get message() {
    return this.#message;
  }
  get checkedAt() {
    return this.#checkedAt;
  }
  get busy() {
    return this.#busy;
  }

  check = async (silent = false) => {
    if (!this.#enabled || this.#busy) return;
    this.#phase = 'checking';
    this.#message = '';
    try {
      this.#info = await call('check_update');
      this.#checkedAt = clockTime();
      this.#phase = this.#info ? 'available' : 'current';
    } catch (cause) {
      this.#info = null;
      this.#phase = 'idle';
      if (!silent) this.#message = errorMessage(cause);
    }
  };

  /**
   * Downloads, verifies and starts the installer in one native call; on
   * success Traygent exits and the installer restarts it. A failure returns
   * to `available` with its message, so the same button retries.
   */
  install = async () => {
    if (!this.#enabled || this.#busy || !this.#info) return;
    this.#phase = 'downloading';
    this.#message = '';
    this.#progress = { downloaded: 0, total: null, stage: 'downloading' };
    const onProgress = channel<DownloadProgress>((progress) => {
      // Stages only move forward, and never once the call has failed.
      const current = INSTALL_PHASES.indexOf(this.#phase);
      if (current < 0 || INSTALL_PHASES.indexOf(progress.stage) < current)
        return;
      this.#phase = progress.stage;
      if (progress.stage === 'downloading') this.#progress = progress;
    });
    try {
      await call('install_update', { onProgress });
      this.#phase = 'installing';
    } catch (cause) {
      this.#phase = 'available';
      this.#message = errorMessage(cause);
    }
  };
}
