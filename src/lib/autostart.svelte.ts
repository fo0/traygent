import { errorMessage } from './format';
import { call } from './ipc';
import type { StartupStatus } from './types';

/** One short line for a status the switch cannot show by itself. */
export function startupNote(
  current: StartupStatus | null,
  reading: boolean,
): string {
  if (!current) return reading ? 'Status wird gelesen …' : 'Status unbekannt.';
  switch (current.status) {
    case 'disabledByWindows':
      return 'In Windows deaktiviert – unter Einstellungen → Apps → Autostart einschalten.';
    case 'unknownApproval':
      return 'Windows-Freigabe unbekannt – unter Einstellungen → Apps → Autostart prüfen.';
    case 'conflict':
      return 'Ein anderer Eintrag nutzt diesen Autostart. Es wird nichts überschrieben.';
    case 'unsupported':
      return 'Windows-Autostart ist nur unter Windows verfügbar.';
    default:
      return '';
  }
}

/**
 * Windows autostart while the settings dialog is open. The switch only
 * stages a choice; „Speichern“ writes it with `apply`. Repair actions act at
 * once. Lives as long as the dialog, so a tab switch keeps the staged choice.
 */
export class Autostart {
  current = $state<StartupStatus | null>(null);
  /** The staged switch position; null while it matches Windows. */
  pending = $state<boolean | null>(null);
  busy = $state(false);
  /** The first read finished, successfully or not. */
  loaded = $state(false);
  /** A failed read or repair action; a failed save goes to the dialog. */
  error = $state('');
  #native: boolean;
  #disposed = false;

  constructor(native: boolean) {
    this.#native = native;
  }

  get editable() {
    return (
      this.current?.status === 'enabled' || this.current?.status === 'disabled'
    );
  }
  /** What the switch shows: the staged choice, else what Windows reports. */
  get checked() {
    return this.pending ?? this.current?.status === 'enabled';
  }

  stage = (enabled: boolean) => {
    this.pending =
      this.editable && enabled !== (this.current?.status === 'enabled')
        ? enabled
        : null;
  };

  /** Re-reads Windows; a staged choice survives while it still differs. */
  read = async () => {
    if (!this.#native || this.busy || this.#disposed) return;
    this.busy = true;
    this.error = '';
    try {
      const result = await call('autostart_status');
      if (this.#disposed) return;
      this.current = result;
      if (this.pending !== null) this.stage(this.pending);
    } catch (cause) {
      if (this.#disposed) return;
      this.current = null;
      this.pending = null;
      this.error = errorMessage(cause);
    } finally {
      if (!this.#disposed) {
        this.busy = false;
        this.loaded = true;
      }
    }
  };

  /** Writes the staged choice; rejects with a user-facing message. */
  apply = async () => {
    const enabled = this.pending;
    if (enabled === null || !this.#native) return;
    this.pending = null;
    const result = await this.#write(enabled);
    if (result.status !== (enabled ? 'enabled' : 'disabled'))
      throw new Error(
        startupNote(result, false) ||
          'Windows-Autostart wurde nicht übernommen.',
      );
  };

  /** „Autostarteintrag entfernen“: removes only Traygent's own entry, at once. */
  remove = async () => {
    this.pending = null;
    try {
      await this.#write(false);
    } catch (cause) {
      if (!this.#disposed) this.error = errorMessage(cause);
    }
  };

  openWindowsSettings = async () => {
    try {
      await call('open_startup_settings');
    } catch (cause) {
      if (!this.#disposed) this.error = errorMessage(cause);
    }
  };

  dispose() {
    this.#disposed = true;
  }

  async #write(enabled: boolean) {
    this.busy = true;
    this.error = '';
    try {
      const result = await call('set_autostart', { enabled });
      if (!this.#disposed) this.current = result;
      return result;
    } catch (cause) {
      // A failed write or readback is uncertain: no optimistic state is kept.
      if (!this.#disposed) this.current = null;
      throw cause;
    } finally {
      if (!this.#disposed) this.busy = false;
    }
  }
}
