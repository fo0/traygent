<script lang="ts">
  import QuotaBar from './QuotaBar.svelte';
  import { quotaColor, type ProfileView } from './quotaDisplay';

  let { profiles }: { profiles: ProfileView[] } = $props();
</script>

<!-- Per shown profile (#94): one label line, its bars stacked below. -->
{#each profiles as profile (profile.id)}
  <span
    class="quota-profile"
    class:start={profile.start}
    data-profile={profile.id}
  >
    <span class="quota-line" title={profile.hint}>
      {#if profile.name}<span class="quota-name">{profile.name}:</span>{/if}
      <span class="quota-metrics"
        >{#each profile.rows as row (row.metric)}<span
            class="quota-metric"
            data-metric={row.metric}
            data-level={row.level ?? 'unknown'}
            style:--metric={quotaColor(row.metric)}
            >{row.label} <b class="quota-figure">{row.value}</b></span
          >{/each}</span
      >
      {#if profile.countdown}<span class="quota-countdown"
          >{profile.countdown}</span
        >{/if}
    </span>
    <span class="quota-bars">
      {#each profile.rows as row (row.metric)}
        <QuotaBar {row} tone="metric" />
      {/each}
    </span>
  </span>
{/each}

<style>
  .quota-profile {
    display: grid;
    gap: 3px;
    min-width: 0;
  }
  .quota-line {
    /* Keep the tooltip above the radio's transparent hit area. */
    position: relative;
    display: flex;
    align-items: baseline;
    gap: 4px;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    color: var(--text-2);
    font-size: 11px;
    font-weight: 400;
    line-height: 1.2;
    font-variant-numeric: tabular-nums;
  }
  /* The name gives way first, then the figures; the countdown stays whole. */
  .quota-name {
    flex: 0 1000 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 500;
  }
  .start .quota-name {
    color: var(--text);
    font-weight: 620;
  }
  .quota-metrics {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* The separator is decoration; the description carries the figures. */
  .quota-metric + .quota-metric::before {
    content: '·';
    margin-inline: 4px;
    color: var(--text-3);
  }
  .quota-metric {
    color: var(--metric);
  }
  .quota-figure {
    font-weight: 550;
  }
  [data-level='warning'] .quota-figure {
    color: var(--wait);
  }
  [data-level='critical'] .quota-figure {
    color: var(--fail);
    font-weight: 700;
  }
  [data-level='unknown'] .quota-figure {
    color: var(--text-3);
    font-weight: 400;
  }
  .quota-countdown {
    flex-shrink: 0;
    margin-inline-start: auto;
    color: var(--text-3);
  }
  .quota-bars {
    display: grid;
    gap: 2px;
  }
  @container (max-width: 325px) {
    .quota-line {
      gap: 3px;
      font-size: 10.5px;
    }
    .quota-metric + .quota-metric::before {
      margin-inline: 3px;
    }
  }
  @media (forced-colors: active) {
    .quota-line,
    .quota-name,
    .quota-metric::before,
    .quota-metric,
    .quota-figure,
    .quota-countdown {
      color: inherit;
    }
  }
</style>
