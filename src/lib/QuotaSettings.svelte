<script lang="ts">
  import QuotaTile from './QuotaTile.svelte';
  import type { AgentQuotas } from './agentQuotas.svelte';
  import { quotaView } from './quotaDisplay';
  import {
    configuredQuotaBars,
    QUOTA_LABELS,
    QUOTA_METRICS,
    type QuotaMetric,
  } from './quotaTypes';
  import type { SettingsDraft } from './settingsDraft.svelte';
  import type { AgentCatalogEntry } from './types';

  let {
    entry,
    draft,
    quotas,
  }: {
    entry: AgentCatalogEntry;
    draft: SettingsDraft;
    quotas: AgentQuotas;
  } = $props();

  const agent = $derived(entry.id);
  const enabled = $derived(configuredQuotaBars(draft.value, agent));
  // Disabled rows keep their catalog positions while the enabled rows follow the saved order.
  const rows = $derived.by(() => {
    const ordered = [...enabled];
    return QUOTA_METRICS[agent].map((metric) =>
      enabled.includes(metric) ? (ordered.shift() ?? metric) : metric,
    );
  });
  const view = $derived(quotaView(quotas.read(agent), enabled, quotas.now));

  function toggle(metric: QuotaMetric, show: boolean) {
    if (metric === 'week') return;
    draft.setQuotaBars(
      agent,
      rows.filter((id) => (id === metric ? show : enabled.includes(id))),
    );
  }

  function move(metric: QuotaMetric, direction: -1 | 1) {
    const next = [...enabled];
    const index = next.indexOf(metric);
    const target = index + direction;
    const other = next[target];
    if (index < 0 || !other) return;
    next[target] = metric;
    next[index] = other;
    draft.setQuotaBars(agent, next);
  }
</script>

<section
  class="settings-group quota-settings"
  aria-labelledby="quota-settings-heading"
>
  <h3 id="quota-settings-heading">Kontingentbalken</h3>
  <p class="field-help">
    Die Woche bleibt bei allen CLIs sichtbar. Weitere Balken erscheinen bei der
    Auswahl.
  </p>
  {#each rows as metric (metric)}
    {@const index = enabled.indexOf(metric)}
    {@const label = QUOTA_LABELS[metric]}
    <div class="quota-settings-row">
      <label
        class="env-toggle"
        title={metric === 'week' ? 'Immer sichtbar' : undefined}
      >
        <input
          type="checkbox"
          checked={index >= 0}
          disabled={metric === 'week'}
          onchange={(event) => toggle(metric, event.currentTarget.checked)}
        />{label}
      </label>
      {#if index >= 0}
        <button
          type="button"
          class="icon-button small"
          disabled={index === 0}
          title={label + ' nach oben'}
          aria-label={label + ' nach oben'}
          onclick={() => move(metric, -1)}>↑</button
        >
        <button
          type="button"
          class="icon-button small"
          disabled={index === enabled.length - 1}
          title={label + ' nach unten'}
          aria-label={label + ' nach unten'}
          onclick={() => move(metric, 1)}>↓</button
        >
      {/if}
    </div>
  {/each}
  <div class="quota-settings-preview" role="group" aria-label="Vorschau">
    <span class="field-label">Vorschau</span>
    <QuotaTile {agent} label={entry.label} {view} />
  </div>
</section>
