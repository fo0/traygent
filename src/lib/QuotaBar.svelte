<script lang="ts">
  import { quotaColor, type QuotaRow } from './quotaDisplay';

  let {
    row,
    tone = 'state',
  }: {
    row: QuotaRow;
    /**
     * `state`: the fill takes the state colour (compact tiles). `metric`: the
     * metric's colour with a short end cap in the state colour from the
     * warning share on (the selected agent's rows, #94).
     */
    tone?: 'state' | 'metric';
  } = $props();
</script>

<span
  class="quota-track"
  class:unknown={row.used === null}
  data-metric={row.metric}
  style:--metric={tone === 'metric' ? quotaColor(row.metric) : undefined}
>
  {#if row.used !== null}
    <span
      class="quota-fill"
      class:metric={tone === 'metric'}
      class:warning={row.level === 'warning'}
      class:critical={row.level === 'critical'}
      style:width={`${row.used}%`}
    ></span>
  {/if}
</span>

<style>
  .quota-track {
    display: block;
    box-sizing: border-box;
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
  .quota-fill.metric {
    --cap: 5px;
    background: var(--metric);
  }
  .quota-fill.metric.warning {
    background: linear-gradient(
      to left,
      var(--wait) 0 var(--cap),
      var(--metric) var(--cap)
    );
  }
  .quota-fill.metric.critical {
    background: linear-gradient(
      to left,
      var(--fail) 0 var(--cap),
      var(--metric) var(--cap)
    );
  }
  .quota-track.unknown {
    background: transparent;
    border-bottom: 1px dashed var(--text-3);
    border-radius: 0;
  }
  @media (forced-colors: active) {
    .quota-track {
      forced-color-adjust: none;
      background: GrayText;
    }
    .quota-fill,
    .quota-fill.warning,
    .quota-fill.critical,
    .quota-fill.metric.warning,
    .quota-fill.metric.critical {
      background: CanvasText;
    }
    .quota-track.unknown {
      background: transparent;
      border-color: GrayText;
    }
  }
</style>
