<script lang="ts">
  import { DEFAULT_QUOTA_REFRESH_MINUTES } from './quotaTypes';
  import type { SettingsDraft } from './settingsDraft.svelte';

  let { draft }: { draft: SettingsDraft } = $props();
  const minutes = $derived(
    draft.value.quotaRefreshMinutes ?? DEFAULT_QUOTA_REFRESH_MINUTES,
  );
  const problem = $derived(draft.problem('app.quotaRefreshMinutes'));
</script>

<section class="settings-group" aria-labelledby="app-quota-heading">
  <h3 id="app-quota-heading">Kontingente</h3>
  <div class="field-grid">
    <label for="quota-refresh-minutes">Abfrageintervall (Minuten)</label>
    <div class="field-cell">
      <input
        id="quota-refresh-minutes"
        class="text-field"
        type="number"
        min="1"
        max="60"
        step="1"
        value={Number.isNaN(minutes) ? '' : minutes}
        aria-invalid={problem ? 'true' : undefined}
        aria-describedby={problem ? 'quota-refresh-error' : undefined}
        oninput={(event) => {
          const minutes = event.currentTarget.valueAsNumber;
          if (
            draft.saved.quotaRefreshMinutes === undefined &&
            minutes === DEFAULT_QUOTA_REFRESH_MINUTES
          )
            delete draft.value.quotaRefreshMinutes;
          else draft.value.quotaRefreshMinutes = minutes;
        }}
      />
      {#if problem}
        <p class="field-error" id="quota-refresh-error">{problem}</p>
      {/if}
    </div>
  </div>
</section>
