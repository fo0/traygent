/**
 * Theme preference. „System“ follows the OS (`prefers-color-scheme`, handled
 * entirely in CSS); „Hell“ and „Dunkel“ override it through `data-theme` on
 * <html>, which 00-tokens.css reads. Stored in localStorage and applied in
 * main.ts before the panel mounts: the CSP forbids an inline script, and the
 * native window starts hidden, so nothing flashes.
 */
export type ThemePreference = 'system' | 'light' | 'dark';

export const THEME_KEY = 'traygent.theme.v1';

export interface ThemeStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

function storage(): ThemeStorage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

function isTheme(value: unknown): value is ThemePreference {
  return value === 'system' || value === 'light' || value === 'dark';
}

/** The saved preference; anything missing, unreadable or unknown is „System“. */
export function readTheme(target = storage()): ThemePreference {
  try {
    const value: unknown = JSON.parse(target?.getItem(THEME_KEY) ?? 'null');
    return isTheme(value) ? value : 'system';
  } catch {
    return 'system';
  }
}

/** Saves a preference („System“ removes the key); false if it could not. */
export function rememberTheme(
  theme: ThemePreference,
  target = storage(),
): boolean {
  if (!target || !isTheme(theme)) return false;
  try {
    if (theme === 'system') target.removeItem(THEME_KEY);
    else target.setItem(THEME_KEY, JSON.stringify(theme));
    return true;
  } catch {
    return false;
  }
}

/**
 * The preference the document shows. Equals the saved one, except after a
 * choice that could not be saved: the setting then keeps showing what is
 * applied instead of the stale stored value.
 */
export function appliedTheme(
  root: HTMLElement | undefined = globalThis.document?.documentElement,
): ThemePreference {
  const value = root?.getAttribute('data-theme');
  return value === 'light' || value === 'dark' ? value : 'system';
}

/**
 * Applies a preference to the document: `data-theme` for an explicit choice,
 * none for „System“. The `theme-color` metas in index.html follow, so a
 * browser frame matches the chosen theme instead of the OS one.
 */
export function applyTheme(
  theme: ThemePreference,
  root: HTMLElement | undefined = globalThis.document?.documentElement,
): void {
  if (!root) return;
  if (theme === 'system') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', theme);
  const metas = root.querySelectorAll<HTMLMetaElement>(
    'meta[name="theme-color"][data-scheme]',
  );
  for (const meta of metas) {
    const scheme = meta.dataset.scheme;
    meta.media =
      theme === 'system'
        ? `(prefers-color-scheme: ${scheme})`
        : theme === scheme
          ? 'all'
          : 'not all';
  }
}
