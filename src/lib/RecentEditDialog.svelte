<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import Modal from './Modal.svelte';
  import OptionFields from './OptionFields.svelte';
  import PortPicker from './PortPicker.svelte';
  import { catalogEntry, envInfo, folderName, sentOptions } from './agents';
  import { errorMessage } from './format';
  import { normalizePorts } from './ports';
  import type { LaunchOptions, RecentSession, Snapshot } from './types';
  import { argsProblem, launchPortsProblem, modelProblem } from './validation';

  // „Bearbeiten …“ of a „Zuletzt“ row: the options and ports its ▶ sends.
  // Staged like the settings; „Speichern“ stores them without a start.
  let {
    snapshot,
    recent,
    returnFocus,
    onSave,
    onClose,
  }: {
    snapshot: Snapshot;
    recent: RecentSession;
    returnFocus?: () => HTMLElement | null | undefined;
    /** Rejects with the German message; the dialog then stays open. */
    onSave: (options: LaunchOptions, ports: string[]) => Promise<void>;
    onClose: () => void;
  } = $props();

  const entry = $derived(catalogEntry(snapshot, recent.agent));
  const docker = $derived(recent.env === 'docker');
  let options = $state<LaunchOptions>(untrack(() => ({ ...recent.options })));
  let ports = $state<string[]>(untrack(() => [...recent.ports]));
  let error = $state('');
  let saving = $state(false);
  let alive = true;
  onDestroy(() => (alive = false));

  const model = $derived(modelProblem(options.model, recent.agent));
  const args = $derived(argsProblem(options.extraArgs));
  const portsProblem = $derived(launchPortsProblem({ env: recent.env, ports }));
  const invalid = $derived(!!(model || args || portsProblem));
  /** What „Speichern“ would store, compared with what the row has. */
  const next = $derived({
    options: sentOptions(entry, options),
    ports: docker ? normalizePorts(ports) : [],
  });
  const dirty = $derived(
    JSON.stringify(next) !==
      JSON.stringify({
        options: sentOptions(entry, recent.options),
        ports: recent.ports,
      }),
  );
  const blocked = $derived(!dirty || saving || invalid);

  async function save() {
    if (blocked) return;
    saving = true;
    error = '';
    try {
      await onSave($state.snapshot(next.options), [...next.ports]);
      if (alive) onClose();
    } catch (cause) {
      if (alive) error = errorMessage(cause);
    } finally {
      if (alive) saving = false;
    }
  }
</script>

<Modal
  title={folderName(recent.folder)}
  eyebrow={`${(entry?.label ?? recent.agent).toUpperCase()} · ${envInfo(recent.env).label.toUpperCase()}`}
  {returnFocus}
  onClose={() => !saving && onClose()}
>
  <form
    novalidate
    onsubmit={(event) => {
      event.preventDefault();
      void save();
    }}
  >
    <div class="modal-body">
      {#if entry}
        <OptionFields
          {entry}
          {options}
          onChange={(patch) => Object.assign(options, patch)}
          prefix="recent"
          modelProblem={model}
          argsProblem={args}
        >
          {#if docker}
            <span class="field-label" aria-hidden="true">Ports</span>
            <PortPicker
              id="recent-ports"
              label="Ports"
              presets={snapshot.portPresets}
              value={ports}
              onChange={(value) => (ports = value)}
            />
          {/if}
        </OptionFields>
      {/if}
    </div>
    {#if error}<div class="dialog-alert danger" role="alert">
        {error}
      </div>{/if}
    <div class="modal-actions">
      <button
        type="button"
        class="secondary-button"
        disabled={saving}
        onclick={() => onClose()}>Abbrechen</button
      ><!-- Off via aria-disabled: it keeps focus while saving. --><button
        type="submit"
        class="primary-button"
        aria-disabled={blocked ? 'true' : undefined}
        title={invalid
          ? 'Ungültige Eingabe'
          : dirty || saving
            ? undefined
            : 'Keine Änderungen'}
        >{saving ? 'Wird gespeichert …' : 'Speichern'}</button
      >
    </div>
  </form>
</Modal>
