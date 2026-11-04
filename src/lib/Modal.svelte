<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import { trapFocus } from './focus';

  let {
    title,
    eyebrow,
    wide = false,
    returnFocus,
    onClose,
    children,
  }: {
    title: string;
    /** Small caps line above the title. */
    eyebrow: string;
    /** Settings layout: a wider dialog with a tab strip. */
    wide?: boolean;
    /** Opener that gets focus back when the dialog closes. */
    returnFocus?: (() => HTMLElement | null | undefined) | undefined;
    /** Close button and Escape. */
    onClose: () => void;
    children: Snippet;
  } = $props();
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === 'Escape') onClose();
  }}
/>

<div class="modal-backdrop" role="presentation">
  <div
    class="modal"
    class:settings-modal={wide}
    role="dialog"
    aria-modal="true"
    aria-labelledby="modal-title"
    tabindex="-1"
    use:trapFocus={{ returnFocus }}
  >
    <div class="modal-heading">
      <div>
        <span class="eyebrow">{eyebrow}</span>
        <h2 id="modal-title">{title}</h2>
      </div>
      <button
        class="icon-button"
        aria-label="Dialog schließen"
        onclick={() => onClose()}><Icon name="close" size={19} /></button
      >
    </div>
    {@render children()}
  </div>
</div>
