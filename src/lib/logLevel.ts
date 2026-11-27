import type { AppSettings, LogLevel } from './types';

/** „Protokoll“ (#109) in display order; mirrors `Level` in src-tauri/src/app_log.rs. */
export const LOG_LEVELS: readonly { value: LogLevel; label: string }[] = [
  { value: 'info', label: 'Info' },
  { value: 'debug', label: 'Debug' },
];

/** The tooltip of „Debug“: what the level changes besides the log. */
export const DEBUG_TOOLTIP =
  'Ausführliches Protokoll; Profil-Anmeldung im sichtbaren Terminal';

/**
 * Whether a profile sign-in opens the visible terminal (#41): only at the
 * saved level „Debug“; otherwise it runs hidden.
 */
export function visibleSignIn(settings: AppSettings | undefined) {
  return settings?.logLevel === 'debug';
}
