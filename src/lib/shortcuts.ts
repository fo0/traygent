export type Shortcut =
  { kind: 'refresh' } | { kind: 'folder' } | { kind: 'escape' };

type KeyPress = Pick<
  KeyboardEvent,
  'key' | 'code' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'
>;

export interface ShortcutActions {
  /** A dialog is open; it owns the keyboard (Escape, focus trap). */
  blocked: () => boolean;
  /** Refreshes when the panel's refresh button would be enabled. */
  refresh: () => void;
  /**
   * Ctrl+L: focuses the composer's folder field, like a browser's address
   * bar; returns whether the field was there to take focus.
   */
  folder: () => boolean;
  /**
   * Escape on `target`; returns whether something handled it. Menus and
   * dialogs handle their own Escape before this runs.
   */
  escape: (target: EventTarget | null) => boolean;
}

/** A Latin letter by its key; other layouts fall back to the physical key. */
function isLetter(event: KeyPress, letter: string) {
  const key = event.key.toLowerCase();
  return /^[a-z]$/.test(key)
    ? key === letter
    : event.code === `Key${letter.toUpperCase()}`;
}

/** Maps a key press to a panel shortcut, independent of focus and state. */
export function shortcutFor(event: KeyPress): Shortcut | null {
  const command = event.ctrlKey || event.metaKey;
  // Alt (and AltGr, reported as Ctrl+Alt on Windows) types characters.
  if (event.altKey) return null;
  if (event.key === 'F5' || (command && isLetter(event, 'r')))
    return { kind: 'refresh' };
  if (command && !event.shiftKey && isLetter(event, 'l'))
    return { kind: 'folder' };
  if (!command && event.key === 'Escape') return { kind: 'escape' };
  return null;
}

/**
 * Window keydown handler for the panel shortcuts. A reload is always
 * suppressed, since it would drop open dialogs and typed input; while a dialog
 * is open nothing else runs. A held refresh key refreshes once.
 */
export function createShortcuts(actions: ShortcutActions) {
  return (event: KeyboardEvent) => {
    const shortcut = shortcutFor(event);
    if (!shortcut) return;
    const handled = event.defaultPrevented;
    if (shortcut.kind === 'refresh') {
      event.preventDefault();
      // A held F5 repeats keydown; one refresh per press, not one per repeat.
      if (event.repeat) return;
    }
    if (handled || event.isComposing || actions.blocked()) return;
    if (shortcut.kind === 'refresh') actions.refresh();
    else if (shortcut.kind === 'folder') {
      if (actions.folder()) event.preventDefault();
    } else if (actions.escape(event.target)) event.preventDefault();
  };
}
