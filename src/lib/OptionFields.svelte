<script lang="ts">
  import type { Snippet } from 'svelte';
  import Segmented, { type Segment } from './Segmented.svelte';
  import { choiceTitle, effortNeedsModel } from './agents';
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

<div class="field-grid">
  <label for={`${prefix}-model`}>Modell</label>
  <div class="field-cell">
    {#if entry.freeModel}
      <input
        id={`${prefix}-model`}
        class="text-field"
        type="text"
        list={`${prefix}-models-${id}`}
        placeholder="Standard"
        spellcheck="false"
        autocomplete="off"
        title={choiceTitle('Modell', entry.models, options.model)}
        aria-invalid={modelProblem ? 'true' : undefined}
        aria-describedby={modelProblem ? `${prefix}-model-error` : undefined}
        value={options.model === 'default' ? '' : options.model}
        oninput={(event) =>
          onChange({ model: event.currentTarget.value.trim() || 'default' })}
      />
      <datalist id={`${prefix}-models-${id}`}>
        {#each entry.models.filter((m) => m.id !== 'default') as model (model.id)}<option
            value={model.id}
            label={model.label}
          ></option>{/each}
      </datalist>
    {:else}
      <select
        id={`${prefix}-model`}
        class="text-field"
        title={choiceTitle('Modell', entry.models, options.model)}
        value={options.model}
        onchange={(event) => onChange({ model: event.currentTarget.value })}
      >
        {#each entry.models as model (model.id)}<option value={model.id}
            >{model.label}</option
          >{/each}
      </select>
    {/if}
    {#if modelProblem}<p id={`${prefix}-model-error`} class="field-error">
        {modelProblem}
      </p>{/if}
  </div>
  {#if entry.efforts.length}
    <label for={`${prefix}-effort`}>Denken</label>
    <!-- A disabled control shows no tooltip everywhere: its cell carries it too. -->
    <div class="field-cell" title={effortTitle}>
      <select
        id={`${prefix}-effort`}
        class="text-field"
        title={effortTitle}
        disabled={effortOff}
        value={options.effort}
        onchange={(event) => onChange({ effort: event.currentTarget.value })}
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
