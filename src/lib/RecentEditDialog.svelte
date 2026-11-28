<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import Dropdown from './Dropdown.svelte';
  import Modal from './Modal.svelte';
  import OptionFields from './OptionFields.svelte';
  import PortPicker from './PortPicker.svelte';
  import Segmented, { type Segment } from './Segmented.svelte';
  import {
    catalogEntry,
    effortProblem,
    envInfo,
    folderName,
    sentOptions,
    shimStart,
  } from './agents';
  import { errorMessage } from './format';
  import { normalizePorts } from './ports';
  import { DEFAULT_PROFILE, recentProfileItems } from './profiles';
  import { isSaveShortcut } from './shortcuts';
  import type { LaunchOptions, RecentSession, Snapshot } from './types';
  import {
    argsProblem,
    launchPortsProblem,
    modelProblem,
    shimArgsProblem,
  } from './validation';

  // „Bearbeiten …“ of a „Workspaces“ row: the options, ports and Docker
  // socket (Docker), profile (when supported) and
  // „Modell & Denken“ mode its ▶ sends. Staged like the settings;
  // „Speichern“ stores them without a start.
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
    onSave: (
      options: LaunchOptions,
      ports: string[],
      profile: string | undefined,
      keepAgentModel: boolean,
      dockerSocket: boolean,
    ) => Promise<void>;
    onClose: () => void;
  } = $props();

  const entry = $derived(catalogEntry(snapshot, recent.agent));
  const docker = $derived(recent.env === 'docker');
  let options = $state<LaunchOptions>(untrack(() => ({ ...recent.options })));
  let ports = $state<string[]>(untrack(() => [...recent.ports]));
  let socket = $state(untrack(() => !!recent.dockerSocket));
  /** „Standard“ first; `[]` when the entry has no profile to choose. */
  const profiles = $derived(
    recentProfileItems(snapshot, recent.agent, recent.env, recent.profile),
  );
  /** A profile no longer listed shows (and saves) as „Standard“. */
  let profile = $state(
    untrack(() => {
      const stored = recent.profile;
      return profiles.some((item) => item.id === stored) && stored
        ? stored
        : DEFAULT_PROFILE;
    }),
  );
  type ModelMode = 'traygent' | 'agent';
  const MODES: Segment<ModelMode>[] = [
    {
      id: 'traygent',
      label: 'Traygent',
      hint: 'Jeder Start setzt Modell und Denkstufe von hier',
    },
    {
      id: 'agent',
      label: 'Agent',
      hint: 'Ohne Modell und Denkstufe: der Agent nimmt, was er zuletzt nutzte',
    },
  ];
  let mode = $state<ModelMode>(
    untrack(() => (recent.keepAgentModel ? 'agent' : 'traygent')),
  );
  const keepAgentModel = $derived(mode === 'agent');
  let error = $state('');
  let saving = $state(false);
  let alive = true;
  onDestroy(() => (alive = false));

  const model = $derived(modelProblem(options.model, recent.agent));
  /** As the row's ▶ checks them: an npm `.cmd` shim re-parses the arguments. */
  const args = $derived(
    argsProblem(options.extraArgs) ||
      (shimStart(snapshot, recent.agent, recent.env)
        ? shimArgsProblem(options.extraArgs)
        : ''),
  );
  const effort = $derived(effortProblem(entry, options));
  const portsProblem = $derived(launchPortsProblem({ env: recent.env, ports }));
  const invalid = $derived(!!(model || args || effort || portsProblem));
  /** What „Speichern“ would store, compared with what the row has. */
  const next = $derived({
    options: sentOptions(entry, options),
    ports: docker ? normalizePorts(ports) : [],
    profile:
      profiles.length && profile !== DEFAULT_PROFILE ? profile : undefined,
    keepAgentModel,
    dockerSocket: docker && socket,
  });
  const dirty = $derived(
    JSON.stringify(next) !==
      JSON.stringify({
        options: sentOptions(entry, recent.options),
        ports: recent.ports,
        profile: recent.profile,
        keepAgentModel: !!recent.keepAgentModel,
        dockerSocket: !!recent.dockerSocket,
      }),
  );
  const blocked = $derived(!dirty || saving || invalid);

  async function save() {
    if (blocked) return;
    saving = true;
    error = '';
    try {
      await onSave(
        $state.snapshot(next.options),
        [...next.ports],
        next.profile,
        next.keepAgentModel,
        next.dockerSocket,
      );
      if (alive) onClose();
    } catch (cause) {
      if (alive) error = errorMessage(cause);
    } finally {
      if (alive) saving = false;
    }
  }
  /** Ctrl+S saves like „Speichern“; never the webview's own save. */
  function saveKey(event: KeyboardEvent) {
    if (!isSaveShortcut(event) || event.isComposing) return;
    event.preventDefault();
    if (!event.repeat) void save();
  }
</script>

<svelte:window onkeydown={saveKey} />

<Modal
  title={folderName(recent.folder)}
  eyebrow={`${(entry?.label ?? recent.agent).toUpperCase()} · ${envInfo(recent.env).label.toUpperCase()}`}
  {returnFocus}
  closeDisabled={saving}
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
          modelOff={keepAgentModel ? 'Modell & Denken vom Agent' : ''}
        >
          {#snippet lead()}
            <span class="field-label" aria-hidden="true">Modell & Denken</span>
            <Segmented
              name="recent-model-mode"
              label="Modell & Denken"
              items={MODES}
              value={mode}
              onChange={(value) => (mode = value)}
            />
          {/snippet}
          {#if profiles.length}
            <label for="recent-profile">Profil</label>
            <div class="field-cell">
              <Dropdown
                id="recent-profile"
                label="Profil"
                items={profiles}
                value={profile}
                onChange={(id) => (profile = id)}
                variant="field"
              />
            </div>
          {/if}
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
      {#if docker}
        <label
          class="switch-row"
          title="Mountet den Docker-Socket des Hosts in den Container – der Agent steuert damit den Host-Docker und kann jeden Host-Ordner einbinden"
        >
          <input type="checkbox" class="switch-input" bind:checked={socket} />
          <span class="switch" aria-hidden="true"></span>
          <span>Docker-Socket durchreichen</span>
        </label>
      {/if}
    </div>
    {#if error || effort}<div class="dialog-alert danger" role="alert">
        {error || effort}
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
        aria-keyshortcuts="Control+S"
        title={invalid
          ? 'Ungültige Eingabe'
          : dirty || saving
            ? 'Speichern (Strg+S)'
            : 'Keine Änderungen'}
        >{saving ? 'Wird gespeichert …' : 'Speichern'}</button
      >
    </div>
  </form>
</Modal>
