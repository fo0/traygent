/**
 * Keyboard and pointer behaviour of a popup menu (WAI-ARIA menu button
 * pattern): arrow keys, Home and End move focus between the items, Escape
 * closes and returns focus to the menu button, Tab closes and lets focus move
 * on from the button, and a click or focus outside closes the menu.
 */

import { hasDom } from './focus';

/** Where focus goes when the menu opens. */
export type MenuFocus = 'first' | 'last';

/**
 * The last pointer press on the page and whether it closed a popup (a menu,
 * a popover, the folder suggestions) from outside. Its click then does
 * nothing else: a „Zuletzt“ row is one big start, and a press that only
 * dismissed a popup over the panel must never start a session.
 */
export class Presses {
  #at = Number.NaN;
  #closed = false;
  /** A press began (`pointerdown`, at its `timeStamp`). */
  press(at: number) {
    if (at === this.#at) return;
    this.#at = at;
    this.#closed = false;
  }
  /** The press at `at`, by default the one under way, closed a popup. */
  close(at = this.#at) {
    this.press(at);
    this.#closed = true;
  }
  /** The click ends a press that closed a popup; a key's click (detail 0) never does. */
  closedBy(click: { detail: number }) {
    return click.detail > 0 && this.#closed;
  }
}
const presses = new Presses();
// Captured on the document before any popup's own listener (they register
// when they open), so every press starts out as one that closed nothing.
if (hasDom())
  document.addEventListener(
    'pointerdown',
    (event) => presses.press(event.timeStamp),
    true,
  );
/** The press under way closed a popup that listens to `blur`, not presses. */
export const pressClosedPopup = () => presses.close();
/** This click ends a press that closed a popup: it must not act as well. */
export const closedPopup = (click: { detail: number }) =>
  presses.closedBy(click);

/**
 * The item index a key moves focus to, wrapping at both ends, or null for a
 * key the menu leaves alone. `current` is -1 while no item has focus.
 */
export function menuIndex(
  key: string,
  current: number,
  count: number,
): number | null {
  if (count < 1) return null;
  switch (key) {
    case 'ArrowDown':
      return current < 0 ? 0 : (current + 1) % count;
    case 'ArrowUp':
      return current < 0 ? count - 1 : (current - 1 + count) % count;
    case 'Home':
    case 'PageUp':
      return 0;
    case 'End':
    case 'PageDown':
      return count - 1;
    default:
      return null;
  }
}

/** Arrow keys on the closed menu button open it; Enter and Space click it. */
export function openingFocus(key: string): MenuFocus | null {
  return key === 'ArrowDown' ? 'first' : key === 'ArrowUp' ? 'last' : null;
}

export interface MenuOptions {
  /** The menu button: Escape and Tab return focus to it. */
  trigger: HTMLElement | undefined;
  /** The item that receives focus when the menu opens. */
  initial?: MenuFocus;
  /** Closes the menu; called at most once per opening. */
  onClose: () => void;
}

/** Enabled items in document order. */
function items(node: HTMLElement) {
  return [
    ...node.querySelectorAll<HTMLElement>(
      '[role="menuitem"], [role="menuitemradio"], [role="menuitemcheckbox"]',
    ),
  ].filter(
    (item) =>
      item.getAttribute('aria-disabled') !== 'true' &&
      !(item instanceof HTMLButtonElement && item.disabled),
  );
}

/**
 * Svelte action for an open `role="menu"` element: focuses the first (or
 * last) item and handles the keys and outside interaction above. Escape calls
 * `preventDefault`, so the panel's own Escape handling skips that key press.
 */
export function menu(node: HTMLElement, options: MenuOptions) {
  let current = options;
  let closed = false;
  const inside = (target: EventTarget | null) =>
    target instanceof Node &&
    (node.contains(target) || !!current.trigger?.contains(target));
  const close = (refocus: boolean) => {
    if (closed) return;
    closed = true;
    if (refocus) current.trigger?.focus();
    current.onClose();
  };
  const onKey = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      close(true);
      return;
    }
    // Focus moves back to the button first, so the browser's own Tab step
    // continues from there instead of from an item that is about to vanish.
    if (event.key === 'Tab') {
      close(true);
      return;
    }
    const list = items(node);
    const active = document.activeElement;
    const next = menuIndex(
      event.key,
      list.findIndex((item) => item === active),
      list.length,
    );
    if (next === null) return;
    event.preventDefault();
    list[next]?.focus();
  };
  const onPointer = (event: PointerEvent) => {
    if (inside(event.target)) return;
    presses.close(event.timeStamp);
    close(false);
  };
  // Settles after the current update: a menu removed while it holds focus
  // (its row went away) fires `focusout` in the middle of that update, and
  // closing writes state, which Svelte forbids there (`state_unsafe_mutation`).
  const onFocusOut = (event: FocusEvent) => {
    const next = event.relatedTarget;
    queueMicrotask(() => {
      if (!inside(next)) close(false);
    });
  };
  node.addEventListener('keydown', onKey);
  node.addEventListener('focusout', onFocusOut);
  document.addEventListener('pointerdown', onPointer, true);
  const list = items(node);
  (current.initial === 'last' ? list.at(-1) : list[0])?.focus();
  return {
    update(next: MenuOptions) {
      current = next;
    },
    destroy() {
      node.removeEventListener('keydown', onKey);
      node.removeEventListener('focusout', onFocusOut);
      document.removeEventListener('pointerdown', onPointer, true);
    },
  };
}

export interface PopoverOptions {
  /** The button that opened it: Escape returns focus to it. */
  trigger: HTMLElement | undefined;
  /** Closes the popover; called at most once per opening. */
  onClose: () => void;
}

/**
 * Svelte action for an open non-modal popover (`role="dialog"`) with form
 * controls: focuses its first control, Tab moves through it as usual, Escape
 * (inside or on the trigger) closes and returns focus to the trigger (with
 * `preventDefault`, like the menu), Enter on anything but a button never
 * submits an enclosing form, and a click or focus outside closes it.
 */
export function popover(node: HTMLElement, options: PopoverOptions) {
  let current = options;
  let closed = false;
  const inside = (target: EventTarget | null) =>
    target instanceof Node &&
    (node.contains(target) || !!current.trigger?.contains(target));
  const close = (refocus: boolean) => {
    if (closed) return;
    closed = true;
    if (refocus) current.trigger?.focus();
    current.onClose();
  };
  const onEscape = (event: KeyboardEvent) => {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    close(true);
  };
  const onKey = (event: KeyboardEvent) => {
    // Enter on a checkbox would submit the composer and start a session; a
    // button keeps its own Enter (it clicks). No `stopPropagation`: Svelte
    // delegates the controls' own handlers (the custom field's Enter) to the
    // root, which a stop here would cut off.
    if (event.key === 'Enter') {
      if (!(event.target instanceof HTMLButtonElement)) event.preventDefault();
      return;
    }
    onEscape(event);
  };
  const onPointer = (event: PointerEvent) => {
    if (inside(event.target)) return;
    presses.close(event.timeStamp);
    close(false);
  };
  // Settles after the current update, as in `menu`. Focus that moves to no
  // element (a click on padding or text, a removed chip button) stays inside:
  // it returns to the control that lost it, else to the first one.
  const onFocusOut = (event: FocusEvent) => {
    const next = event.relatedTarget;
    const lost = event.target;
    queueMicrotask(() => {
      if (next === null && node.isConnected && !closed) {
        if (inside(document.activeElement)) return;
        const back =
          lost instanceof HTMLElement && lost.isConnected && node.contains(lost)
            ? lost
            : first();
        back?.focus();
        return;
      }
      if (!inside(next)) close(false);
    });
  };
  const first = () =>
    node.querySelector<HTMLElement>('input, button, select, textarea');
  let trigger = current.trigger;
  node.addEventListener('keydown', onKey);
  node.addEventListener('focusout', onFocusOut);
  trigger?.addEventListener('keydown', onEscape);
  document.addEventListener('pointerdown', onPointer, true);
  first()?.focus();
  return {
    update(next: PopoverOptions) {
      current = next;
      if (next.trigger === trigger) return;
      trigger?.removeEventListener('keydown', onEscape);
      trigger = next.trigger;
      trigger?.addEventListener('keydown', onEscape);
    },
    destroy() {
      node.removeEventListener('keydown', onKey);
      node.removeEventListener('focusout', onFocusOut);
      trigger?.removeEventListener('keydown', onEscape);
      document.removeEventListener('pointerdown', onPointer, true);
    },
  };
}
