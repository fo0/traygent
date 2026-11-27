<script lang="ts">
  import { tick } from 'svelte';
  import QuotaTile from './QuotaTile.svelte';
  import type { AgentQuotas } from './agentQuotas.svelte';
  import { quotaView } from './quotaDisplay';
  import {
    configuredQuotaBars,
    QUOTA_LABELS,
    QUOTA_METRICS,
    quotaReadOn,
    quotaReadTip,
    quotaRefreshMinutes,
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
  const reading = $derived(quotaReadOn(draft.value, agent));
  const view = $derived(
    // The preview shows the bars of the agent's lead profile, without a name.
    quotaView(
      [
        {
          id: agent,
          name: '',
          start: true,
          quota: quotas.read(agent),
          updatedAt: quotas.updatedAt(agent),
        },
      ],
      enabled,
      quotas.now,
      reading,
      quotaRefreshMinutes(draft.value),
    ),
  );

  function toggle(metric: QuotaMetric, show: boolean) {
    if (metric === 'week') return;
    draft.setQuotaBars(
      agent,
      rows.filter((id) => (id === metric ? show : enabled.includes(id))),
    );
  }

  async function move(
    metric: QuotaMetric,
    direction: -1 | 1,
    button: HTMLButtonElement,
  ) {
    const next = [...enabled];
    const index = next.indexOf(metric);
    const target = index + direction;
    const other = next[target];
    if (index < 0 || !other) return;
    next[target] = metric;
    next[index] = other;
    draft.setQuotaBars(agent, next);
    // The row moves, and at the end of the list the pressed arrow turns off:
    // the focus stays with the row, on the arrow that still works.
    await tick();
    const row = button.parentElement;
    const keep = button.disabled
      ? row?.querySelector<HTMLButtonElement>('button:not(:disabled)')
      : button;
    if (keep && document.activeElement !== keep) keep.focus();
  }
</script>

<section
  class="settings-group quota-settings"
  aria-labelledby="quota-settings-heading"
>
  <h3 id="quota-settings-heading">Kontingent</h3>
  <!-- Opt-in per agent (ADR-0014); the tooltip says which sign-in it uses. -->
  <label class="switch-row" title={quotaReadTip(agent)}>
    <input
      type="checkbox"
      class="switch-input"
      title={quotaReadTip(agent)}
      checked={reading}
      onchange={(event) =>
        draft.setQuotaRead(agent, event.currentTarget.checked)}
    />
    <span class="switch" aria-hidden="true"></span>
    <span>Kontingent abfragen</span>
  </label>
  {#each rows as metric (metric)}
    {@const index = enabled.indexOf(metric)}
    {@const label = QUOTA_LABELS[metric]}
    <div class="quota-settings-row" class:off={!reading}>
      <label
        class="env-toggle"
        title={metric === 'week' ? 'Bei allen CLIs immer sichtbar' : undefined}
      >
        <input
          type="checkbox"
          checked={index >= 0}
          disabled={metric === 'week' || !reading}
          onchange={(event) => toggle(metric, event.currentTarget.checked)}
        />{label}
      </label>
      {#if index >= 0}
        <button
          type="button"
          class="icon-button small"
          disabled={!reading || index === 0}
          title={label + ' nach oben'}
          aria-label={label + ' nach oben'}
          onclick={(event) => void move(metric, -1, event.currentTarget)}
          >↑</button
        >
        <button
          type="button"
          class="icon-button small"
          disabled={!reading || index === enabled.length - 1}
          title={label + ' nach unten'}
          aria-label={label + ' nach unten'}
          onclick={(event) => void move(metric, 1, event.currentTarget)}
          >↓</button
        >
      {/if}
    </div>
  {/each}
  <div class="quota-settings-preview" role="group" aria-label="Vorschau">
    <span class="field-label">Vorschau</span>
    <QuotaTile {agent} label={entry.label} {view} />
  </div>
</section>
