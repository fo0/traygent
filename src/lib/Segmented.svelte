<script module lang="ts">
  import type { IconName } from './Icon.svelte';
  import type { ProbeState } from './types';

  export interface Segment<T extends string> {
    id: T;
    label: string;
    icon?: IconName | undefined;
    /** A small probe dot after the label, e.g. the agent in this environment. */
    dot?: ProbeState | undefined;
    disabled?: boolean | undefined;
    /** Tooltip and accessible description, e.g. „Bald verfügbar“. */
    hint?: string | undefined;
    /** Drawn in the warning colour, e.g. a permission that skips every prompt. */
    warn?: boolean | undefined;
  }
</script>

<script lang="ts" generics="T extends string">
  import Icon from './Icon.svelte';

  // Native radios: arrow keys, focus and disabled items come from the browser.
  let {
    name,
    label,
    items,
    value,
    onChange,
    landing = false,
    prominent = false,
    collapse = false,
  }: {
    /** The radio group's name; unique per page. */
    name: string;
    /** Accessible name of the group; never shown. */
    label: string;
    items: readonly Segment<T>[];
    value: T;
    onChange: (value: T) => void;
    /** The checked radio is the page's focus landing (`focus.ts → landing`). */
    landing?: boolean;
    /**
     * Larger from 480 px on (36 px segments instead of 28, 14.5 px names,
     * 18 px marks), for the row a form leads with (the composer's agents).
     */
    prominent?: boolean;
    /**
     * Only the checked segment shows its name and takes the room; the others
     * shrink to their mark and dot. Names stay in the accessible names and
     * the tooltips.
     */
    collapse?: boolean;
  } = $props();
  const hintId = (item: Segment<T>) => `${name}-${item.id}-hint`;
</script>

<div
  class={['segmented', prominent && 'prominent', collapse && 'collapse']}
  role="radiogroup"
  aria-label={label}
>
  {#each items as item (item.id)}
    <label
      class="segment"
      title={item.hint}
      data-warn={item.warn ? 'true' : undefined}
    >
      <input
        type="radio"
        {name}
        value={item.id}
        checked={value === item.id}
        disabled={item.disabled}
        aria-describedby={item.hint ? hintId(item) : undefined}
        data-focus-landing={landing && value === item.id ? true : undefined}
        onchange={() => onChange(item.id)}
      />
      {#if item.icon}<Icon name={item.icon} size={prominent ? 18 : 14} />{/if}
      <span class="segment-label">{item.label}</span>
      {#if item.dot}<span class="status-dot" data-state={item.dot}></span>{/if}
    </label>
    {#if item.hint}<span id={hintId(item)} hidden>{item.hint}</span>{/if}
  {/each}
</div>
