<script lang="ts">
  import Icon from './Icon.svelte';
  import PortsChip from './PortsChip.svelte';
  import {
    PERMISSION_ICONS,
    SESSION_HINTS,
    SESSION_LABELS,
    choiceTitle,
    effortChoices,
    permissionChoice,
  } from './agents';
  import type { Composer } from './composerState.svelte';
  import { effortLevel } from './optionMarks';
  import type {
    AgentCatalogEntry,
    EnvId,
    PortPreset,
    SessionMode,
  } from './types';

  // Compact chips for the selected agent; each changes the next start only.
  let {
    entry,
    composer,
    presets,
    invalidModel = '',
    onEnter,
  }: {
    entry: AgentCatalogEntry;
    composer: Composer;
    /** The port checkboxes of the Docker-only „Ports“ chip. */
    presets: readonly PortPreset[];
    /** Why the typed model is refused; marks the chip. */
    invalidModel?: string;
    /** Enter on a select starts like everywhere else in the composer. */
    onEnter: () => void;
  } = $props();
  const options = $derived(composer.options);
  const permission = $derived(permissionChoice(entry, options.permission));
  const models = $derived(entry.models.filter((m) => m.id !== 'default'));
  /** Hidden while the model cannot carry a thinking level (nothing is sent). */
  const efforts = $derived(effortChoices(entry, options.model));
  const knownEffort = $derived(
    efforts.some((choice) => choice.id === options.effort),
  );
  const sessions = $derived(composer.sessions);
  const listId = $derived(`composer-models-${entry.id}`);
  const modelTitle = $derived(
    invalidModel || choiceTitle('Modell', entry.models, options.model),
  );
  const enter = (event: KeyboardEvent) => {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    onEnter();
  };
  const sessionTitle = (mode: SessionMode, env: EnvId) =>
    `Session: ${SESSION_HINTS[mode]}${env === 'docker' && mode !== 'new' ? ' (im Container)' : ''}`;
</script>

<div class="option-chips" role="group" aria-label="Optionen">
  {#if entry.freeModel}
    <label
      class="chip chip-field"
      data-invalid={invalidModel ? 'true' : undefined}
      title={modelTitle}
      ><Icon name="cpu" size={13} /><input
        class="chip-input"
        type="text"
        list={listId}
        aria-label="Modell"
        aria-invalid={invalidModel ? 'true' : undefined}
        placeholder="Modell"
        spellcheck="false"
        autocomplete="off"
        value={options.model === 'default' ? '' : options.model}
        oninput={(event) =>
          (composer.options.model =
            event.currentTarget.value.trim() || 'default')}
      /></label
    >
    <datalist id={listId}>
      {#each models as model (model.id)}<option
          value={model.id}
          label={model.label}
        ></option>{/each}
    </datalist>
  {:else}
    <label class="chip" title={modelTitle}
      ><Icon name="cpu" size={13} /><select
        class="chip-select"
        aria-label="Modell"
        value={options.model}
        onkeydown={enter}
        onchange={(event) =>
          (composer.options.model = event.currentTarget.value)}
      >
        {#each entry.models as model (model.id)}<option value={model.id}
            >{model.label}</option
          >{/each}
      </select><Icon name="down" size={11} class="chip-caret" /></label
    >
  {/if}
  {#if efforts.length}
    <label class="chip" title={choiceTitle('Denken', efforts, options.effort)}
      ><Icon
        name="signal"
        size={14}
        level={effortLevel(options.effort)}
      /><select
        class="chip-select"
        aria-label="Denken"
        value={options.effort}
        onkeydown={enter}
        onchange={(event) =>
          (composer.options.effort = event.currentTarget.value)}
      >
        {#if !knownEffort}<option value={options.effort}
            >{options.effort}</option
          >{/if}
        {#each efforts as effort (effort.id)}<option value={effort.id}
            >{effort.label}</option
          >{/each}
      </select><Icon name="down" size={11} class="chip-caret" /></label
    >
  {/if}
  <!-- Icon only: the label lives in the accessible name and the tooltip. -->
  <button
    type="button"
    class="chip chip-button chip-icon"
    data-warn={permission.danger ? 'true' : undefined}
    aria-label={`Freigabe: ${permission.label}`}
    title={`Freigabe: ${permission.label}${permission.hint ? ` – ${permission.hint}` : ''} (klicken zum Wechseln)`}
    onclick={() => composer.cyclePermission()}
    ><Icon name={PERMISSION_ICONS[permission.id]} size={14} /></button
  >
  {#if sessions.length > 1}
    <label class="chip" title={sessionTitle(composer.session, composer.env)}
      ><Icon name="history" size={13} /><select
        class="chip-select"
        aria-label="Session"
        value={composer.session}
        onkeydown={enter}
        onchange={(event) =>
          (composer.session = event.currentTarget.value as SessionMode)}
      >
        {#each sessions as mode (mode)}<option value={mode}
            >{SESSION_LABELS[mode]}</option
          >{/each}
      </select><Icon name="down" size={11} class="chip-caret" /></label
    >
  {/if}
  {#if composer.env === 'docker'}
    <PortsChip
      {presets}
      value={composer.ports}
      onChange={(next) => (composer.ports = next)}
    />
  {/if}
</div>
