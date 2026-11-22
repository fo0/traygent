<script lang="ts">
  import Icon from './Icon.svelte';
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
  {#each view.rows as row (row.metric)}
    <span class="quota-meter" data-metric={row.metric}>
      {#if active}
        <span class="quota-caption">
          <span>{row.label}</span>
          <span class="quota-value">
            {#if row.metric === 'week' && row.countdown}
              {row.countdown} ·
            {/if}
            <b>{row.free}</b>
          </span>
        </span>
      {/if}
      <span class="quota-track" class:unknown={row.used === null}>
        {#if row.used !== null}
          <span
            class="quota-fill"
            class:warning={row.used >= 70}
            class:critical={row.used >= 90}
            style:width={`${row.used}%`}
          ></span>
        {/if}
      </span>
    </span>
  {/each}
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
  .quota-updated,
  .quota-caption {
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
  .status-dot {
    width: 6px;
    height: 6px;
  }
  .quota-updated,
  .quota-caption {
    gap: 4px;
    color: var(--text-2);
    font-size: 11px;
    font-weight: 400;
    line-height: 1.2;
    font-variant-numeric: tabular-nums;
  }
  .quota-updated {
    /* Keep the refresh tooltip above the radio's transparent hit area. */
    position: relative;
    margin-inline-start: auto;
  }
  .quota-meter {
    display: grid;
    gap: 3px;
  }
  .quota-caption {
    justify-content: space-between;
    color: var(--text-3);
  }
  .quota-caption b {
    font-weight: 550;
    color: var(--text-2);
  }
  .quota-track {
    display: block;
    height: 3px;
    border-radius: 2px;
    background: var(--border-strong);
  }
  .quota-fill {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--ok);
  }
  .quota-fill.warning {
    background: var(--wait);
  }
  .quota-fill.critical {
    background: var(--fail);
  }
  .quota-track.unknown {
    background: transparent;
    border-bottom: 1px dashed var(--text-3);
    border-radius: 0;
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
    .quota-track {
      forced-color-adjust: none;
      background: GrayText;
    }
    .quota-fill,
    .quota-fill.warning,
    .quota-fill.critical {
      background: CanvasText;
    }
    .quota-track.unknown {
      background: transparent;
      border-color: GrayText;
    }
    .quota-updated,
    .quota-caption,
    .quota-caption b {
      color: inherit;
    }
  }
</style>
