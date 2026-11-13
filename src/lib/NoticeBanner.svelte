<script lang="ts">
  import Icon from './Icon.svelte';
  import type { NoticeTone } from './StatusBanners.svelte';
  import { focusAfterNotice, focusLost, neighbourNotice } from './focus';

  let {
    text,
    tone,
    actionLabel,
    onAction,
    navigates = true,
    timeout,
    paused = false,
    onDismiss,
  }: {
    text: string;
    tone: NoticeTone;
    /** Optional follow-up action, e.g. „Rückgängig“. */
    actionLabel?: string | undefined;
    onAction?: (() => void) | undefined;
    /** The action leads elsewhere (arrow icon) rather than acting in place. */
    navigates?: boolean;
    /**
     * Closes after this many milliseconds. A notice with an action waits
     * while hovered or focused; a plain one closes regardless.
     */
    timeout?: number | undefined;
    /** Holds the timeout, e.g. while a dialog hides the notice; restarts in full after. */
    paused?: boolean;
    onDismiss: () => void;
  } = $props();
  let hovered = $state(false);
  let focused = $state(false);
  /** Only an action is worth holding the notice for. */
  const holds = $derived(!!(actionLabel && onAction));

  $effect(() => {
    if (!timeout || paused || (holds && (hovered || focused))) return;
    // Starts over once the pointer and focus have left a notice with an
    // action and no dialog hides it any more.
    const timer = setTimeout(() => onDismiss(), timeout);
    return () => clearTimeout(timer);
  });
  /**
   * The tray popup hides without a `pointerleave`: a hidden or unfocused
   * window holds no notice, so none stays forever.
   */
  function release() {
    hovered = false;
    focused = false;
  }

  /** Focus target if this notice closes while focused, kept while it is listed. */
  let neighbour: HTMLElement | null = null;
  /** The action just ran and places focus itself (the run, the restored card). */
  let actionOwnsFocus = false;

  /** Tracks whether the pointer or the focus is inside the notice. */
  function engagement(node: HTMLElement) {
    const enter = () => (hovered = true);
    const leave = () => (hovered = false);
    const focusIn = () => {
      focused = true;
      neighbour = neighbourNotice(node);
    };
    // Settles after the current update: the list may be removing this notice
    // right now (Chromium fires `focusout` before the node is gone), and
    // writing state in the middle of that update throws
    // `state_unsafe_mutation`. A removed notice that held focus hands it on,
    // unless the action that closed it places focus itself.
    const focusOut = () =>
      queueMicrotask(() => {
        if (node.isConnected)
          focused =
            document.hasFocus() && node.contains(document.activeElement);
        else if (!actionOwnsFocus && focusLost()) focusAfterNotice(neighbour);
      });
    node.addEventListener('pointerenter', enter);
    node.addEventListener('pointerleave', leave);
    node.addEventListener('focusin', focusIn);
    node.addEventListener('focusout', focusOut);
    return {
      destroy() {
        node.removeEventListener('pointerenter', enter);
        node.removeEventListener('pointerleave', leave);
        node.removeEventListener('focusin', focusIn);
        node.removeEventListener('focusout', focusOut);
      },
    };
  }

  /** Moves focus on first, so closing never drops it to <body>. */
  function close(event: MouseEvent) {
    const self = event.currentTarget;
    if (self instanceof HTMLElement && self === document.activeElement) {
      const banner = self.closest<HTMLElement>('.notice-banner');
      focusAfterNotice(banner ? neighbourNotice(banner) : null);
    }
    onDismiss();
  }
  function act() {
    actionOwnsFocus = true;
    // The action closes the notice within this task, if at all.
    setTimeout(() => (actionOwnsFocus = false));
    onAction?.();
  }
</script>

<svelte:window onblur={release} />
<svelte:document onvisibilitychange={() => document.hidden && release()} />

<div
  class={`alert ${tone} notice-banner`}
  role={tone === 'danger' ? 'alert' : 'status'}
  use:engagement
>
  <Icon name={tone === 'subtle' ? 'check' : 'warning'} size={16} /><span
    >{text}</span
  >{#if actionLabel && onAction}<button class="text-button" onclick={act}
      >{actionLabel}{#if navigates}<Icon name="arrow" size={13} />{/if}</button
    >{/if}<button
    class="icon-button small"
    aria-label="Meldung schließen"
    title="Meldung schließen"
    onclick={close}><Icon name="close" size={13} /></button
  >
</div>
