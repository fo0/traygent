<script lang="ts">
  import Segmented, { type Segment } from './Segmented.svelte';
  import { choiceTitle, effortNeedsModel } from './agents';
  import type { SettingsDraft } from './settingsDraft.svelte';
  import type { AgentCatalogEntry, PermissionId } from './types';

  // The staged defaults of one agent: every new start begins from these.
  let { entry, draft }: { entry: AgentCatalogEntry; draft: SettingsDraft } =
    $props();
  const id = $derived(entry.id);
  const options = $derived(draft.value.agents[entry.id]);
  const modelProblem = $derived(draft.problem(`agents.${entry.id}.model`));
  const argsProblem = $derived(draft.problem(`agents.${entry.id}.extraArgs`));
  const knownEffort = $derived(
    entry.efforts.some((choice) => choice.id === options.effort),
  );
  /** Kept as stored, but off: without a model no thinking level is sent. */
  const effortOff = $derived(effortNeedsModel(entry, options.model));
  const effortTitle = $derived(
    effortOff
      ? 'Denken: nur mit gewähltem Modell'
      : choiceTitle('Denken', entry.efforts, options.effort),
  );
  const permissions = $derived<Segment<PermissionId>[]>(
    entry.permissions.map((choice) => ({
      id: choice.id,
      label: choice.label,
      hint: choice.hint || undefined,
      warn: choice.danger,
    })),
  );
</script>

<section class="settings-group" aria-labelledby="defaults-heading">
  <h3 id="defaults-heading">Standards</h3>
  <div class="field-grid">
    <label for="default-model">Modell</label>
    <div class="field-cell">
      {#if entry.freeModel}
        <input
          id="default-model"
          class="text-field"
          type="text"
          list={`default-models-${id}`}
          placeholder="Standard"
          spellcheck="false"
          autocomplete="off"
          title={choiceTitle('Modell', entry.models, options.model)}
          aria-invalid={modelProblem ? 'true' : undefined}
          aria-describedby={modelProblem ? 'default-model-error' : undefined}
          value={options.model === 'default' ? '' : options.model}
          oninput={(event) =>
            (draft.value.agents[id].model =
              event.currentTarget.value.trim() || 'default')}
        />
        <datalist id={`default-models-${id}`}>
          {#each entry.models.filter((m) => m.id !== 'default') as model (model.id)}<option
              value={model.id}
              label={model.label}
            ></option>{/each}
        </datalist>
      {:else}
        <select
          id="default-model"
          class="text-field"
          title={choiceTitle('Modell', entry.models, options.model)}
          bind:value={draft.value.agents[id].model}
        >
          {#each entry.models as model (model.id)}<option value={model.id}
              >{model.label}</option
            >{/each}
        </select>
      {/if}
      {#if modelProblem}<p id="default-model-error" class="field-error">
          {modelProblem}
        </p>{/if}
    </div>
    {#if entry.efforts.length}
      <label for="default-effort">Denken</label>
      <!-- A disabled control shows no tooltip everywhere: its cell carries it too. -->
      <div class="field-cell" title={effortTitle}>
        <select
          id="default-effort"
          class="text-field"
          title={effortTitle}
          disabled={effortOff}
          bind:value={draft.value.agents[id].effort}
        >
          {#if !knownEffort}<option value={options.effort}
              >{options.effort}</option
            >{/if}
          {#each entry.efforts as effort (effort.id)}<option value={effort.id}
              >{effort.label}</option
            >{/each}
        </select>
      </div>
    {/if}
    <span class="field-label" aria-hidden="true">Freigabe</span>
    <Segmented
      name={`default-permission-${id}`}
      label="Freigabe"
      items={permissions}
      value={options.permission}
      onChange={(value) => (draft.value.agents[id].permission = value)}
    />
    <label for="default-args">Extra-Argumente</label>
    <div class="field-cell">
      <input
        id="default-args"
        class="text-field mono"
        type="text"
        placeholder="keine"
        spellcheck="false"
        autocomplete="off"
        title="Durch Leerzeichen getrennt, ohne Anführungszeichen"
        aria-invalid={argsProblem ? 'true' : undefined}
        aria-describedby={argsProblem ? 'default-args-error' : undefined}
        bind:value={draft.value.agents[id].extraArgs}
      />
      {#if argsProblem}<p id="default-args-error" class="field-error">
          {argsProblem}
        </p>{/if}
    </div>
  </div>
</section>
