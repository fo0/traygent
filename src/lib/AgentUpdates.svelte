<script lang="ts">
  import Icon from './Icon.svelte';
  import { agentLabel } from './agents';
  import { agentUpdateText } from './agentUpdates';
  import { minuteClock } from './clock';
  import { dateTime, errorMessage, relativeTime } from './format';
  import type { SettingsDraft } from './settingsDraft.svelte';
  import type { Snapshot } from './types';

  // The checkbox stages with „Speichern“; „Jetzt aktualisieren“ acts at once.
  let {
    snapshot,
    draft,
    onUpdate,
  }: {
    snapshot: Snapshot;
    draft: SettingsDraft;
    /** Starts a run now; rejects with the German message. */
    onUpdate: () => Promise<void>;
  } = $props();
  let starting = $state(false);
  let error = $state('');
  const busy = $derived(starting || snapshot.agentUpdatesRunning);

  async function update() {
    if (busy) return;
    starting = true;
    error = '';
    try {
      await onUpdate();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      starting = false;
    }
  }
</script>

<section
  class="agent-updates"
  aria-labelledby="agent-updates-heading"
  aria-busy={busy}
>
  <h3 id="agent-updates-heading">Agenten</h3>
  <div class="agent-updates-row">
    <label class="agent-updates-option">
      <input type="checkbox" bind:checked={draft.value.agentUpdates} />
      <span>
        <strong>Automatisch aktualisieren</strong>
        <span>Alle 4 Stunden, nur ohne offene Session.</span>
      </span>
    </label>
    <!-- Off via aria-disabled: it keeps focus while the run goes on. -->
    <button
      type="button"
      class="secondary-button"
      aria-disabled={busy ? 'true' : undefined}
      onclick={() => void update()}>Jetzt aktualisieren</button
    >
  </div>
  <!-- Announces each state a row reaches, that row only: the row is rebuilt
       per state and time (an addition), the minute clock only changes text. -->
  <div role="status" aria-atomic="false" aria-relevant="additions">
    {#if snapshot.agentUpdates.length}
      <ul class="agent-update-list" aria-label="Agenten-Updates">
        {#each snapshot.agentUpdates as status (status.agent)}
          {@const state = agentUpdateText(status)}
          {@const id = `agent-update-${status.agent}`}
          <!-- Named by its parts, so the tooltip's detail is in the name. -->
          <li aria-labelledby="{id}-name {id}-state {id}-time">
            {#key `${status.state} ${status.at}`}
              <span id="{id}-name" class="agent-update-name"
                >{agentLabel(snapshot, status.agent)}</span
              >
              <span
                id="{id}-state"
                class="agent-update-state"
                data-tone={state.tone}
                title={state.title}
                >{state.text}{#if state.detail}<span class="sr-only"
                    >{` (${state.detail})`}</span
                  >{/if}</span
              >
              <time
                id="{id}-time"
                datetime={status.at}
                title={dateTime(status.at)}
                >{relativeTime(status.at, $minuteClock)}</time
              >
            {/key}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
  {#if error}<div class="alert danger" role="alert">
      <Icon name="warning" size={14} /><span>{error}</span>
    </div>{/if}
</section>
