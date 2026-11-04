import { describe, expect, it } from 'vitest';
import {
  THEME_KEY,
  appliedTheme,
  applyTheme,
  readTheme,
  rememberTheme,
  type ThemeStorage,
} from './theme';

function memory(): ThemeStorage & { values: Map<string, string> } {
  const values = new Map<string, string>();
  return {
    values,
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => {
      values.set(key, value);
    },
    removeItem: (key) => {
      values.delete(key);
    },
  };
}

/** A synthetic <html> with the two theme-color metas of index.html. */
function documentRoot() {
  const attributes = new Map<string, string>();
  const metas = ['light', 'dark'].map((scheme) => ({
    dataset: { scheme },
    media: `(prefers-color-scheme: ${scheme})`,
  }));
  const root = {
    getAttribute: (name: string) => attributes.get(name) ?? null,
    setAttribute: (name: string, value: string) => attributes.set(name, value),
    removeAttribute: (name: string) => attributes.delete(name),
    querySelectorAll: () => metas,
  };
  return { root: root as unknown as HTMLElement, metas };
}

describe('theme preference storage', () => {
  it('stores an explicit choice and reads it back', () => {
    const target = memory();
    expect(rememberTheme('light', target)).toBe(true);
    expect(target.getItem(THEME_KEY)).toBe('"light"');
    expect(readTheme(target)).toBe('light');
    expect(rememberTheme('dark', target)).toBe(true);
    expect(readTheme(target)).toBe('dark');
  });

  it('removes the key for „System“, the default', () => {
    const target = memory();
    rememberTheme('dark', target);
    expect(rememberTheme('system', target)).toBe(true);
    expect(target.values.has(THEME_KEY)).toBe(false);
    expect(readTheme(target)).toBe('system');
  });

  it('falls back to „System“ for missing, malformed or unknown values', () => {
    const target = memory();
    expect(readTheme(target)).toBe('system');
    for (const raw of ['light', '{', '"sepia"', '42', '["dark"]', 'null']) {
      target.setItem(THEME_KEY, raw);
      expect(readTheme(target)).toBe('system');
    }
  });

  it('never stores an unknown preference', () => {
    const target = memory();
    expect(rememberTheme('sepia' as never, target)).toBe(false);
    expect(target.values.size).toBe(0);
  });

  it('survives a storage that throws', () => {
    const broken: ThemeStorage = {
      getItem: () => {
        throw new Error('denied');
      },
      setItem: () => {
        throw new Error('quota');
      },
      removeItem: () => {
        throw new Error('denied');
      },
    };
    expect(readTheme(broken)).toBe('system');
    expect(rememberTheme('light', broken)).toBe(false);
    expect(rememberTheme('system', broken)).toBe(false);
  });
});

describe('applying the theme', () => {
  it('sets data-theme for an explicit choice and removes it for „System“', () => {
    const { root } = documentRoot();
    applyTheme('light', root);
    expect(root.getAttribute('data-theme')).toBe('light');
    applyTheme('dark', root);
    expect(root.getAttribute('data-theme')).toBe('dark');
    applyTheme('system', root);
    expect(root.getAttribute('data-theme')).toBeNull();
  });

  it('points the theme-color metas at the chosen theme, or back at the OS', () => {
    const { root, metas } = documentRoot();
    const media = () => metas.map((meta) => meta.media);
    applyTheme('dark', root);
    expect(media()).toEqual(['not all', 'all']);
    applyTheme('light', root);
    expect(media()).toEqual(['all', 'not all']);
    applyTheme('system', root);
    expect(media()).toEqual([
      '(prefers-color-scheme: light)',
      '(prefers-color-scheme: dark)',
    ]);
  });

  it('reads back what the document shows, even if it was never saved', () => {
    const { root } = documentRoot();
    expect(appliedTheme(root)).toBe('system');
    applyTheme('light', root);
    expect(appliedTheme(root)).toBe('light');
    root.setAttribute('data-theme', 'sepia');
    expect(appliedTheme(root)).toBe('system');
  });

  it('does nothing without a document', () => {
    expect(() => applyTheme('dark', undefined)).not.toThrow();
    expect(appliedTheme(undefined)).toBe('system');
  });
});
