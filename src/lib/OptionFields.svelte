<script lang="ts">
  import type { Snippet } from 'svelte';
  import Dropdown from './Dropdown.svelte';
  import ModelCombo from './ModelCombo.svelte';
  import Segmented, { type Segment } from './Segmented.svelte';
  import { choiceTitle, effortNeedsModel } from './agents';
  import { toItems } from './choices';
  import type { AgentCatalogEntry, LaunchOptions, PermissionId } from './types';

  // One agent's options as form fields: Einstellungen → Agenten → Standards
  // and „Bearbeiten …“ of a „Zuletzt“ row.
  let {
    entry,
    options,
    onChange,
    prefix,
    modelProblem = '',
    argsProblem = '',
    children,
  }: {
    entry: AgentCatalogEntry;
    options: LaunchOptions;
    /** One changed field; the owner of `options` applies it. */
    onChange: (patch: Partial<LaunchOptions>) => void;
    /** Prefix of the element ids; unique per form on the page. */
    prefix: string;
    modelProblem?: string;
    argsProblem?: string;
    /** Further rows of the same grid, e.g. the ports of a Docker session. */
    children?: Snippet;
  } = $props();
  const id = $derived(entry.id);
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

<div class="field-grid">
  <label for={`${prefix}-model`}>Modell</label>
  <div class="field-cell">
    {#if entry.freeModel}
      <ModelCombo
        id={`${prefix}-model`}
        models={entry.models}
        value={options.model}
        onChange={(model) => onChange({ model })}
        title={choiceTitle('Modell', entry.models, options.model)}
        invalid={modelProblem}
        describedBy={modelProblem ? `${prefix}-model-error` : undefined}
        variant="field"
      />
    {:else}
      <Dropdown
        id={`${prefix}-model`}
        label="Modell"
        items={toItems(entry.models, options.model)}
        value={options.model}
        onChange={(model) => onChange({ model })}
        title={choiceTitle('Modell', entry.models, options.model)}
        variant="field"
      />
    {/if}
    {#if modelProblem}<p id={`${prefix}-model-error`} class="field-error">
        {modelProblem}
      </p>{/if}
  </div>
  {#if entry.efforts.length}
    <label for={`${prefix}-effort`}>Denken</label>
    <!-- A disabled control shows no tooltip everywhere: its cell carries it too. -->
    <div class="field-cell" title={effortTitle}>
      <Dropdown
        id={`${prefix}-effort`}
        label="Denken"
        items={toItems(entry.efforts, options.effort)}
        value={options.effort}
        onChange={(effort) => onChange({ effort })}
        title={effortTitle}
        disabled={effortOff}
        variant="field"
      />
    </div>
  {/if}
  <span class="field-label" aria-hidden="true">Freigabe</span>
  <Segmented
    name={`${prefix}-permission-${id}`}
    label="Freigabe"
    items={permissions}
    value={options.permission}
    onChange={(value) => onChange({ permission: value })}
  />
  <label for={`${prefix}-args`}>Extra-Argumente</label>
  <div class="field-cell">
    <input
      id={`${prefix}-args`}
      class="text-field mono"
      type="text"
      placeholder="keine"
      spellcheck="false"
      autocomplete="off"
      title="Durch Leerzeichen getrennt, ohne Anführungszeichen"
      aria-invalid={argsProblem ? 'true' : undefined}
      aria-describedby={argsProblem ? `${prefix}-args-error` : undefined}
      value={options.extraArgs}
      oninput={(event) => onChange({ extraArgs: event.currentTarget.value })}
    />
    {#if argsProblem}<p id={`${prefix}-args-error`} class="field-error">
        {argsProblem}
      </p>{/if}
  </div>
  {@render children?.()}
</div>
