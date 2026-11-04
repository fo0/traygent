<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';
  import { plural } from './format';

  let {
    id,
    title,
    count,
    unit,
    icon,
    children,
  }: {
    /** The heading's id; the section is labelled by it. */
    id: string;
    title: string;
    /** Left out, the header shows no count. */
    count?: number | undefined;
    /** Singular and plural for the spoken count, e.g. „Session“, „Sessions“. */
    unit?: [one: string, many: string];
    icon?: IconName | undefined;
    /** Header actions at the right end. */
    children?: Snippet;
  } = $props();
</script>

<div class="section-header">
  <h2 {id}>
    {#if icon}<Icon name={icon} size={13} />{/if}{title}
  </h2>
  {#if count !== undefined && unit}<span class="section-count"
      ><span aria-hidden="true">{count}</span><span class="sr-only"
        >{plural(count, ...unit)}</span
      ></span
    >{/if}
  {#if children}<span class="section-actions">{@render children()}</span>{/if}
</div>
