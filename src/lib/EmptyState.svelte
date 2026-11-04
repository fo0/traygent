<script module lang="ts">
  import type { IconName } from './Icon.svelte';

  export interface EmptyAction {
    label: string;
    icon: IconName;
    run: () => void;
  }
</script>

<script lang="ts">
  import Icon from './Icon.svelte';

  let {
    icon,
    iconSize,
    framed = true,
    title,
    text,
    action,
  }: {
    icon: IconName;
    iconSize: number;
    /** Wraps the icon in the round `.empty-icon` badge. */
    framed?: boolean;
    title: string;
    /** At most one short line; left out, the title stands alone. */
    text?: string | undefined;
    action?: EmptyAction | undefined;
  } = $props();
</script>

<div class="empty-state">
  {#if framed}<span class="empty-icon"
      ><Icon name={icon} size={iconSize} /></span
    >{:else}<Icon name={icon} size={iconSize} />{/if}
  <h3>{title}</h3>
  {#if text}<p>{text}</p>{/if}
  {#if action}<button class="text-button" onclick={() => action.run()}
      >{action.label}<Icon name={action.icon} size={14} /></button
    >{/if}
</div>
