<script lang="ts">
  import { onMount } from 'svelte';

  // A confirmation in place of the action it guards (ported from RepoWatch).
  let {
    id,
    title,
    text,
    confirmLabel,
    cancelLabel = 'Abbrechen',
    busyLabel,
    busy = false,
    onConfirm,
    onCancel,
  }: {
    /** Prefix for the label and description IDs; unique per confirmation. */
    id: string;
    title: string;
    text: string;
    /** The action asked for, styled as danger. */
    confirmLabel: string;
    cancelLabel?: string;
    /** Confirm label while the action runs. */
    busyLabel?: string;
    busy?: boolean;
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();
  let cancel = $state<HTMLButtonElement>();

  /**
   * Escape cancels only this confirmation, never the surrounding dialog. While
   * the action runs it is swallowed: the action finishes either way.
   */
  function escapeCancels(node: HTMLElement) {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.stopPropagation();
      event.preventDefault();
      if (!busy) onCancel();
    };
    node.addEventListener('keydown', onKey);
    return { destroy: () => node.removeEventListener('keydown', onKey) };
  }

  // The safe choice gets focus, so Enter right after opening never destroys anything.
  onMount(() => cancel?.focus());
  // While busy both buttons stay focusable (aria-disabled, not disabled), so focus
  // never drops to <body> where Escape would close the surrounding dialog.
  const unlessBusy = (action: () => void) => () => {
    if (!busy) action();
  };
</script>

<div
  class="inline-confirm"
  role="group"
  aria-labelledby={`${id}-title`}
  aria-describedby={`${id}-text`}
  use:escapeCancels
>
  <p>
    <strong id={`${id}-title`}>{title}</strong>
    <span id={`${id}-text`}>{text}</span>
  </p>
  <div class="inline-confirm-actions">
    <button
      type="button"
      class="secondary-button compact"
      bind:this={cancel}
      aria-disabled={busy ? 'true' : undefined}
      onclick={unlessBusy(onCancel)}>{cancelLabel}</button
    >
    <button
      type="button"
      class="danger-button"
      aria-disabled={busy ? 'true' : undefined}
      onclick={unlessBusy(onConfirm)}
      >{busy && busyLabel ? busyLabel : confirmLabel}</button
    >
  </div>
</div>
