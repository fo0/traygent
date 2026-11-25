<script lang="ts">
  import Icon from './Icon.svelte';
  import QuotaBar from './QuotaBar.svelte';
  import QuotaProfileRows from './QuotaProfileRows.svelte';
  import { AGENT_ICONS } from './agents';
  import type { QuotaView } from './quotaDisplay';
  import type { AgentId, ProbeState } from './types';

  let {
    agent,
    label,
    view,
    dot,
    active = true,
    hint,
  }: {
    agent: AgentId;
    label: string;
    view: QuotaView;
    dot?: ProbeState;
    active?: boolean;
    /** On a radio, the label owns the combined probe and quota description. */
    hint?: string;
  } = $props();
</script>

<span
  class="quota-tile"
  class:compact={!active}
  role="img"
  aria-label={`${label} · ${view.hint}`}
  aria-hidden={hint ? true : undefined}
  title={hint ?? view.hint}
>
  <span class="quota-heading">
    <Icon name={AGENT_ICONS[agent]} size={18} />
    {#if active}<strong class="segment-label">{label}</strong>{/if}
    {#if dot}<span class="status-dot" data-state={dot}></span>{/if}
    {#if active && !view.off}
      <span class="quota-updated" title={view.refreshHint}>{view.updated}</span>
    {/if}
  </span>
  {#if active}
    <QuotaProfileRows profiles={view.profiles} />
  {:else if !view.off}
    <!-- Another agent: its weekly bar alone, in state colours. -->
    <span class="quota-meter" data-metric="week"
      ><QuotaBar row={view.week} /></span
    >
  {/if}
</span>

<style>
  .quota-tile {
    align-self: stretch;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .quota-heading,
  .quota-updated {
    display: flex;
    align-items: center;
    white-space: nowrap;
  }
  .quota-heading {
    gap: 6px;
    min-height: 19px;
  }
  .compact .quota-heading {
    flex: 1;
    justify-content: center;
  }
  .segment-label {
    font-weight: 620;
  }
  .quota-meter {
    display: grid;
  }
  .status-dot {
    width: 6px;
    height: 6px;
  }
  .quota-updated {
    /* Keep the refresh tooltip above the radio's transparent hit area. */
    position: relative;
    margin-inline-start: auto;
    gap: 4px;
    color: var(--text-2);
    font-size: 11px;
    font-weight: 400;
    line-height: 1.2;
    font-variant-numeric: tabular-nums;
  }
  @container (max-width: 365px) {
    .quota-heading,
    .quota-updated {
      gap: 3px;
    }
    .quota-tile:not(.compact) .quota-heading > :global(.mark) {
      flex-shrink: 1;
      min-width: 0;
    }
    .segment-label,
    .quota-updated {
      flex-shrink: 0;
    }
  }
  @container (max-width: 325px) {
    .quota-heading {
      gap: 2px;
    }
    .quota-updated {
      font-size: 10.5px;
      gap: 2px;
    }
  }
  @container (max-width: 290px) {
    .quota-tile:not(.compact) .quota-heading > :global(.mark) {
      display: none;
    }
  }
  @media (forced-colors: active) {
    .quota-updated {
      color: inherit;
    }
  }
</style>
