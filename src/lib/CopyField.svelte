<script lang="ts">
  import { onDestroy } from 'svelte';
  import Icon from './Icon.svelte';

  // A read-only command in monospace with a copy button; a failed copy
  // selects the text instead, ready for Ctrl+C.
  let {
    value,
    label,
    title,
  }: {
    value: string;
    /** Accessible name, e.g. „Installationsbefehl“. */
    label: string;
    /** Tooltip, e.g. the docs address. */
    title?: string | undefined;
  } = $props();
  let input = $state<HTMLInputElement>();
  let copied = $state(false);
  let failed = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  onDestroy(() => clearTimeout(timer));

  async function copy() {
    clearTimeout(timer);
    try {
      await navigator.clipboard.writeText(value);
      copied = true;
      failed = false;
    } catch {
      failed = true;
      input?.select();
    }
    timer = setTimeout(() => (copied = failed = false), 3000);
  }
</script>

<div class="copy-field">
  <input
    bind:this={input}
    class="text-field mono"
    readonly
    {value}
    aria-label={label}
    title={title ?? value}
    onfocus={(event) => event.currentTarget.select()}
  />
  <button
    type="button"
    class="icon-button small"
    aria-label={copied ? 'Kopiert' : `${label} kopieren`}
    title={copied ? 'Kopiert' : failed ? 'Mit Strg+C kopieren' : 'Kopieren'}
    onclick={() => void copy()}
    ><Icon name={copied ? 'check' : 'copy'} size={14} /></button
  >
</div>
