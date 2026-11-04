export interface TrapFocusOptions {
  /**
   * Element that receives focus when the dialog closes. Chained dialogs pass
   * the opener of the first dialog so focus returns there, not to a button of
   * a dialog that no longer exists.
   */
  returnFocus?: (() => HTMLElement | null | undefined) | undefined;
}

/** Focus was dropped (the focused element was removed or made inert). */
export function focusLost() {
  const active = document.activeElement;
  return !active || active === document.body || !active.isConnected;
}

/**
 * Where focus lands when nothing better is left: the composer's checked
 * agent, the first control under the app bar (RepoWatch lands on its search
 * field there). Exactly one element per page carries `data-focus-landing`.
 */
export function landing() {
  return document.querySelector<HTMLElement>('[data-focus-landing]');
}

/** The first control of the notice next to this one, after it first. */
export function neighbourNotice(node: HTMLElement) {
  for (const sibling of [node.nextElementSibling, node.previousElementSibling])
    if (sibling?.matches('.notice-banner'))
      return sibling.querySelector<HTMLElement>('button');
  return null;
}

/**
 * Where focus goes when the notice holding it closes: the neighbour found
 * while it was still listed, if it is still there, else the page's landing.
 */
export function focusAfterNotice(neighbour: HTMLElement | null) {
  (neighbour?.isConnected ? neighbour : landing())?.focus();
}

/** Focus moves only in the panel; the unit tests run without a DOM. */
export const hasDom = () => typeof document !== 'undefined';

/** Removed or disabled elements cannot take focus back. */
export function usable(target: HTMLElement | null | undefined) {
  return (
    !!target?.isConnected &&
    !(target instanceof HTMLButtonElement && target.disabled)
  );
}

/**
 * Restores focus once the page behind the dialog is interactive again: the
 * dialog's `destroy()` runs while the background is still `inert`, where
 * `focus()` is silently ignored. An opener that was removed or disabled
 * meanwhile (e.g. by a running operation) hands focus to the page's landing.
 * Never steals focus that was placed deliberately in the meantime, and
 * leaves it to a dialog that replaced this one.
 */
function restoreFocus(target: HTMLElement | null | undefined, frames = 3) {
  if (!focusLost()) return;
  if (document.querySelector('[role="dialog"][aria-modal="true"]')) return;
  const next = usable(target) ? target : landing();
  if (!next) return;
  if (next.closest('[inert]')) {
    if (frames > 0)
      requestAnimationFrame(() => restoreFocus(target, frames - 1));
    return;
  }
  next.focus();
}

/** A small modal focus loop, including focus restoration when the dialog closes. */
export function trapFocus(node: HTMLElement, options: TrapFocusOptions = {}) {
  const active = document.activeElement;
  const previous = active instanceof HTMLElement ? active : null;
  let returnFocus = options.returnFocus;
  const focusable = () =>
    [
      ...node.querySelectorAll<HTMLElement>(
        'button:not(:disabled), input:not(:disabled), select:not(:disabled), summary, a[href], [tabindex="0"]',
      ),
    ].filter(
      (item) =>
        !item.closest('details:not([open])') || item.tagName === 'SUMMARY',
    );
  queueMicrotask(() => (focusable()[0] ?? node).focus());
  const onKey = (event: KeyboardEvent) => {
    if (event.key !== 'Tab') return;
    const items = focusable();
    const first = items[0];
    const last = items.at(-1);
    if (!first || !last) {
      event.preventDefault();
      node.focus();
      return;
    }
    if (
      event.shiftKey &&
      (document.activeElement === first || document.activeElement === node)
    ) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };
  node.addEventListener('keydown', onKey);
  return {
    update(next: TrapFocusOptions = {}) {
      returnFocus = next.returnFocus;
    },
    destroy() {
      node.removeEventListener('keydown', onKey);
      const target = returnFocus?.() ?? previous;
      // Runs after the current flush, when `inert` has been lifted.
      queueMicrotask(() => restoreFocus(target));
    },
  };
}
