import { describe, expect, it, vi } from 'vitest';
import {
  createShortcuts,
  isSaveShortcut,
  shortcutFor,
  type ShortcutActions,
} from './shortcuts';

const press = (key: string, extra: Partial<KeyboardEvent> = {}) => ({
  key,
  code: '',
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...extra,
});

describe('isSaveShortcut', () => {
  it('takes Ctrl+S and ⌘S in any case and layout, never with Alt or Shift', () => {
    expect(isSaveShortcut(press('s', { ctrlKey: true }))).toBe(true);
    expect(isSaveShortcut(press('S', { metaKey: true }))).toBe(true);
    // Cyrillic layout: the physical S key; Dvorak: the key labelled S.
    expect(isSaveShortcut(press('ы', { ctrlKey: true, code: 'KeyS' }))).toBe(
      true,
    );
    expect(isSaveShortcut(press('o', { ctrlKey: true, code: 'KeyS' }))).toBe(
      false,
    );
    expect(isSaveShortcut(press('s'))).toBe(false);
    expect(isSaveShortcut(press('s', { ctrlKey: true, altKey: true }))).toBe(
      false,
    );
    expect(isSaveShortcut(press('S', { ctrlKey: true, shiftKey: true }))).toBe(
      false,
    );
  });
  it('is no panel shortcut, so it never reaches the panel handler', () => {
    expect(shortcutFor(press('s', { ctrlKey: true }))).toBeNull();
  });
});

describe('shortcutFor', () => {
  it('maps F5 and Ctrl+R (any case, any layout) to refresh', () => {
    expect(shortcutFor(press('F5'))).toEqual({ kind: 'refresh' });
    expect(shortcutFor(press('r', { ctrlKey: true }))).toEqual({
      kind: 'refresh',
    });
    expect(shortcutFor(press('R', { ctrlKey: true, shiftKey: true }))).toEqual({
      kind: 'refresh',
    });
    // Cyrillic layout: the physical R key still reloads a webview.
    expect(shortcutFor(press('к', { ctrlKey: true, code: 'KeyR' }))).toEqual({
      kind: 'refresh',
    });
    // Dvorak: the key labelled R is what counts, not its position.
    expect(shortcutFor(press('p', { ctrlKey: true, code: 'KeyR' }))).toBeNull();
    expect(shortcutFor(press('r'))).toBeNull();
  });
  it('maps Ctrl+L (any layout) to the folder field, never with Shift', () => {
    expect(shortcutFor(press('l', { ctrlKey: true }))).toEqual({
      kind: 'folder',
    });
    expect(shortcutFor(press('l', { metaKey: true }))).toEqual({
      kind: 'folder',
    });
    expect(shortcutFor(press('д', { ctrlKey: true, code: 'KeyL' }))).toEqual({
      kind: 'folder',
    });
    expect(
      shortcutFor(press('L', { ctrlKey: true, shiftKey: true })),
    ).toBeNull();
    expect(shortcutFor(press('l'))).toBeNull();
    expect(shortcutFor(press('l', { ctrlKey: true, altKey: true }))).toBeNull();
  });
  it('maps Ctrl+1 … Ctrl+9 (any layout) to a workspace row, never with Shift', () => {
    expect(shortcutFor(press('1', { ctrlKey: true }))).toEqual({
      kind: 'workspace',
      index: 0,
    });
    expect(shortcutFor(press('9', { metaKey: true }))).toEqual({
      kind: 'workspace',
      index: 8,
    });
    // AZERTY: the top-row key types „&“, its physical key is still Digit1.
    expect(shortcutFor(press('&', { ctrlKey: true, code: 'Digit1' }))).toEqual({
      kind: 'workspace',
      index: 0,
    });
    expect(shortcutFor(press('0', { ctrlKey: true }))).toBeNull();
    expect(shortcutFor(press('1'))).toBeNull();
    expect(
      shortcutFor(
        press('!', { ctrlKey: true, shiftKey: true, code: 'Digit1' }),
      ),
    ).toBeNull();
    expect(shortcutFor(press('1', { ctrlKey: true, altKey: true }))).toBeNull();
  });
  it('maps a plain Escape and leaves Alt combinations to the layout', () => {
    expect(shortcutFor(press('Escape'))).toEqual({ kind: 'escape' });
    expect(shortcutFor(press('Escape', { ctrlKey: true }))).toBeNull();
    expect(shortcutFor(press('F5', { altKey: true }))).toBeNull();
  });
});

function keydown(key: string, extra: Partial<KeyboardEvent> = {}) {
  const event = {
    ...press(key),
    repeat: false,
    isComposing: false,
    defaultPrevented: false,
    target: null,
    ...extra,
    preventDefault: vi.fn(),
  };
  return event as unknown as KeyboardEvent & {
    preventDefault: ReturnType<typeof vi.fn>;
  };
}

function actions(overrides: Partial<ShortcutActions> = {}) {
  return {
    blocked: () => false,
    refresh: vi.fn(),
    folder: vi.fn(() => true),
    workspace: vi.fn(() => true),
    escape: vi.fn(() => true),
    ...overrides,
  };
}

describe('createShortcuts', () => {
  it('always suppresses the webview reload and refreshes once per press', () => {
    const handlers = actions();
    const handle = createShortcuts(handlers);
    const first = keydown('F5');
    handle(first);
    expect(first.preventDefault).toHaveBeenCalled();
    const repeat = keydown('F5', { repeat: true });
    handle(repeat);
    expect(repeat.preventDefault).toHaveBeenCalled();
    expect(handlers.refresh).toHaveBeenCalledTimes(1);
  });

  it('runs nothing while a dialog is open, but still blocks the reload', () => {
    const handlers = actions({ blocked: () => true });
    const handle = createShortcuts(handlers);
    const reload = keydown('r', { ctrlKey: true });
    handle(reload);
    expect(reload.preventDefault).toHaveBeenCalled();
    handle(keydown('Escape'));
    expect(handlers.refresh).not.toHaveBeenCalled();
    expect(handlers.escape).not.toHaveBeenCalled();
  });

  it('focuses the folder field and prevents Ctrl+L only when it was there', () => {
    const handlers = actions();
    const focused = keydown('l', { ctrlKey: true });
    createShortcuts(handlers)(focused);
    expect(handlers.folder).toHaveBeenCalledTimes(1);
    expect(focused.preventDefault).toHaveBeenCalled();
    const missing = keydown('l', { ctrlKey: true });
    createShortcuts(actions({ folder: vi.fn(() => false) }))(missing);
    expect(missing.preventDefault).not.toHaveBeenCalled();
    const inDialog = actions({ blocked: () => true });
    createShortcuts(inDialog)(keydown('l', { ctrlKey: true }));
    expect(inDialog.folder).not.toHaveBeenCalled();
  });

  it('starts a workspace row once per press and only when it exists', () => {
    const handlers = actions();
    const handle = createShortcuts(handlers);
    const first = keydown('2', { ctrlKey: true });
    handle(first);
    expect(handlers.workspace).toHaveBeenCalledWith(1);
    expect(first.preventDefault).toHaveBeenCalled();
    const repeat = keydown('2', { ctrlKey: true, repeat: true });
    handle(repeat);
    expect(repeat.preventDefault).toHaveBeenCalled();
    expect(handlers.workspace).toHaveBeenCalledTimes(1);
    const missing = keydown('9', { ctrlKey: true });
    createShortcuts(actions({ workspace: vi.fn(() => false) }))(missing);
    expect(missing.preventDefault).not.toHaveBeenCalled();
    const inDialog = actions({ blocked: () => true });
    createShortcuts(inDialog)(keydown('1', { ctrlKey: true }));
    expect(inDialog.workspace).not.toHaveBeenCalled();
  });

  it('prevents Escape only when something handled it', () => {
    const unhandled = actions({ escape: vi.fn(() => false) });
    const event = keydown('Escape');
    createShortcuts(unhandled)(event);
    expect(event.preventDefault).not.toHaveBeenCalled();
    const handled = keydown('Escape');
    createShortcuts(actions())(handled);
    expect(handled.preventDefault).toHaveBeenCalled();
  });
});
