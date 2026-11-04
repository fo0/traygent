<script lang="ts">
  import type { Action } from 'svelte/action';
  import Icon from './Icon.svelte';
  import PortPicker from './PortPicker.svelte';
  import { popover } from './menu';
  import { EXPOSED, exposedIp, portListProblem } from './ports';
  import type { PortPreset } from './types';

  // The composer's „Ports“ chip (Docker only): opens the picker in a popover;
  // the choice applies to the next start only.
  let {
    presets,
    value,
    onChange,
  }: {
    presets: readonly PortPreset[];
    value: readonly string[];
    onChange: (next: string[]) => void;
  } = $props();

  /** Space kept between the popover and the content area's edge. */
  const EDGE = 8;
  /** A popover never gets shorter than this; the content area scrolls then. */
  const MIN_HEIGHT = 120;

  let open = $state(false);
  let trigger = $state<HTMLButtonElement>();
  /** The first IP other than 127.0.0.1: that port is reachable from the LAN. */
  const exposed = $derived(
    value.map(exposedIp).find((ip) => ip !== null) ?? null,
  );
  /** The chip shows the ports glyph, then the count and an exposing IP. */
  const short = $derived(
    `${value.length || ''}${exposed ? ` · ${exposed}` : ''}`,
  );
  const text = $derived(`Ports${short ? ` ${short}` : ''}`);
  const problem = $derived(portListProblem(value));
  const title = $derived(
    problem ||
      `Ports: ${value.length ? value.join(', ') : 'keine'}${exposed ? ` · ${EXPOSED}` : ''}`,
  );

  /**
   * Keeps the popover inside the scrolling content area, above the footer:
   * it opens upwards when it does not fit below and there is more room
   * above, and scrolls inside when it fits neither way. Placed again on
   * every change and whenever the chip row or the popover resizes: a longer
   * chip label („Ports 3 · 0.0.0.0“) wraps the row after the change.
   */
  const place: Action<HTMLElement, readonly string[]> = (node) => {
    measure(node);
    // Settles once: the same room sets the same max-height, no new resize.
    // A wrapped chip row shortens the popover after the change: the focused
    // control is kept in view then too.
    const observer = new ResizeObserver(() => {
      measure(node);
      reveal(node);
    });
    const row = node.closest('.composer-actions');
    if (row) observer.observe(row);
    observer.observe(node);
    return {
      destroy: () => observer.disconnect(),
      update() {
        measure(node);
        reveal(node);
      },
    };
  };
  /** The focused control (the custom field after Enter) stays in view. */
  function reveal(node: HTMLElement) {
    const focused = document.activeElement;
    if (focused instanceof HTMLElement && node.contains(focused))
      focused.scrollIntoView({ block: 'nearest' });
  }
  function measure(node: HTMLElement) {
    const content = node.closest('.content')?.getBoundingClientRect();
    const row = node.closest('.composer-actions')?.getBoundingClientRect();
    if (!content || !row) return;
    const below = content.bottom - row.bottom - EDGE;
    const above = row.top - content.top - EDGE;
    const up = node.scrollHeight > below && above > below;
    node.classList.toggle('up', up);
    node.style.maxHeight = `${Math.max(MIN_HEIGHT, Math.floor(up ? above : below))}px`;
  }
</script>

<span class="ports-chip">
  <!-- Symbol first: the word „Ports“ lives in the accessible name. -->
  <button
    bind:this={trigger}
    type="button"
    class={['chip', 'chip-button', !short && 'chip-icon']}
    data-invalid={problem ? 'true' : undefined}
    data-warn={exposed && !problem ? 'true' : undefined}
    aria-label={exposed ? `${text}, ${EXPOSED}` : text}
    aria-haspopup="dialog"
    aria-expanded={open}
    {title}
    onclick={() => (open = !open)}
    ><Icon name="ports" size={14} />{#if short}<span>{short}</span>{/if}</button
  >
  {#if open}
    <div
      class="menu port-popover"
      role="dialog"
      aria-label="Ports"
      use:place={value}
      use:popover={{ trigger, onClose: () => (open = false) }}
    >
      <PortPicker
        id="composer-ports"
        label="Ports"
        {presets}
        {value}
        {onChange}
      />
    </div>
  {/if}
</span>
