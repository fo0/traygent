<script lang="ts">
  import Dropdown from './Dropdown.svelte';
  import Icon from './Icon.svelte';
  import ModelCombo from './ModelCombo.svelte';
  import PortsChip from './PortsChip.svelte';
  import ProfileChip from './ProfileChip.svelte';
  import {
    PERMISSION_ICONS,
    SESSION_HINTS,
    SESSION_LABELS,
    choiceTitle,
    effortChoices,
    permissionChoice,
  } from './agents';
  import { toItems } from './choices';
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
    /** Enter on a closed dropdown starts like everywhere else in the composer. */
    onEnter: () => void;
  } = $props();
  const options = $derived(composer.options);
  const permission = $derived(permissionChoice(entry, options.permission));
  /** Hidden while the model cannot carry a thinking level (nothing is sent). */
  const efforts = $derived(effortChoices(entry, options.model));
  const sessions = $derived(composer.sessions);
  /** Lokal and an agent with an extra profile only. */
  const profiles = $derived(composer.profiles);
  const modelTitle = $derived(
    invalidModel || choiceTitle('Modell', entry.models, options.model),
  );
  const sessionTitle = (mode: SessionMode, env: EnvId) =>
    `Session: ${SESSION_HINTS[mode]}${env === 'docker' && mode !== 'new' ? ' (im Container)' : ''}`;
</script>

<div class="option-chips" role="group" aria-label="Optionen">
  {#if entry.freeModel}
    <ModelCombo
      id={`composer-model-${entry.id}`}
      models={entry.models}
      value={options.model}
      onChange={(model) => (composer.options.model = model)}
      title={modelTitle}
      invalid={invalidModel}
      {onEnter}
    />
  {:else}
    <Dropdown
      id={`composer-model-${entry.id}`}
      label="Modell"
      items={toItems(entry.models, options.model)}
      value={options.model}
      onChange={(model) => (composer.options.model = model)}
      title={modelTitle}
      {onEnter}
      >{#snippet lead()}<Icon name="cpu" size={13} />{/snippet}</Dropdown
    >
  {/if}
  {#if efforts.length}
    <Dropdown
      id="composer-effort"
      label="Denken"
      items={toItems(efforts, options.effort)}
      value={options.effort}
      onChange={(effort) => (composer.options.effort = effort)}
      title={choiceTitle('Denken', efforts, options.effort)}
      {onEnter}
      >{#snippet lead()}<Icon
          name="signal"
          size={14}
          level={effortLevel(options.effort)}
        />{/snippet}</Dropdown
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
    <Dropdown
      id="composer-session"
      label="Session"
      items={sessions.map((mode) => ({
        id: mode,
        label: SESSION_LABELS[mode],
        title: SESSION_HINTS[mode],
      }))}
      value={composer.session}
      onChange={(mode) => (composer.session = mode as SessionMode)}
      title={sessionTitle(composer.session, composer.env)}
      {onEnter}
      >{#snippet lead()}<Icon name="history" size={13} />{/snippet}</Dropdown
    >
  {/if}
  {#if profiles.length}
    <ProfileChip
      {profiles}
      value={composer.profile}
      onChange={(id) => (composer.profile = id)}
      {onEnter}
    />
  {/if}
  {#if composer.env === 'docker'}
    <PortsChip
      {presets}
      value={composer.ports}
      onChange={(next) => (composer.ports = next)}
    />
  {/if}
</div>
