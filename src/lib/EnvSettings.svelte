<script lang="ts">
  import PortPicker from './PortPicker.svelte';
  import Segmented, { type Segment } from './Segmented.svelte';
  import Icon from './Icon.svelte';
  import { AGENT_ICONS, ENVIRONMENTS, SOON } from './agents';
  import { NO_ENVIRONMENT, lastEnv } from './environments';
  import type { SettingsDraft } from './settingsDraft.svelte';
  import type { AgentId, EnvId, Snapshot, TerminalId } from './types';

  // „Umgebung“: which environments the composer offers, where and in which
  // window a start runs by default, the Docker image per agent and the
  // default ports. Everything is staged until „Speichern“.
  let { snapshot, draft }: { snapshot: Snapshot; draft: SettingsDraft } =
    $props();
  const settings = $derived(draft.value);
  /** The default is one of the shown environments; one alone needs no row. */
  const environments = $derived<Segment<EnvId>[]>(
    ENVIRONMENTS.filter((env) => settings.environments.includes(env.id)).map(
      (env) => ({
        id: env.id,
        label: env.label,
        icon: env.icon,
        disabled: env.id === 'wsl',
        hint: env.id === 'wsl' ? SOON : undefined,
      }),
    ),
  );
  const envsProblem = $derived(draft.problem('environments'));
  /** Why a toggle is locked or what it means; the last usable one stays on. */
  const toggleHint = (env: EnvId) =>
    lastEnv(settings.environments, env)
      ? NO_ENVIRONMENT
      : env === 'wsl'
        ? SOON
        : undefined;
  const agents = $derived<Segment<AgentId>[]>(
    snapshot.catalog.map((entry) => ({
      id: entry.id,
      label: entry.label,
      icon: AGENT_ICONS[entry.id],
    })),
  );
  const terminals: Segment<TerminalId>[] = [
    {
      id: 'windowsTerminal',
      label: 'Windows Terminal',
      icon: 'terminal',
      hint: 'Neuer Tab in Windows Terminal (wt.exe)',
    },
    {
      id: 'console',
      label: 'Konsole',
      icon: 'monitor',
      hint: 'Neues PowerShell-Fenster',
    },
  ];
</script>

<section class="settings-group" aria-labelledby="env-defaults-heading">
  <h3 id="env-defaults-heading">Standard</h3>
  <div class="field-grid">
    <span class="field-label" aria-hidden="true">Umgebungen</span>
    <div class="field-cell">
      <div
        class="env-toggles"
        role="group"
        aria-label="Umgebungen"
        aria-describedby={envsProblem ? 'settings-envs-error' : undefined}
      >
        {#each ENVIRONMENTS as env (env.id)}
          {@const hint = toggleHint(env.id)}
          {@const locked = lastEnv(settings.environments, env.id)}
          <!-- Locked via aria-disabled: it keeps its focus and its reason. -->
          <label class="env-toggle" title={hint}
            ><input
              type="checkbox"
              checked={settings.environments.includes(env.id)}
              aria-disabled={locked ? 'true' : undefined}
              aria-describedby={hint
                ? `settings-env-${env.id}-hint`
                : undefined}
              onclick={(event) => locked && event.preventDefault()}
              onchange={(event) =>
                draft.toggleEnv(env.id, event.currentTarget.checked)}
            /><Icon name={env.icon} size={13} />{env.label}</label
          >
          {#if hint}<span id={`settings-env-${env.id}-hint`} hidden>{hint}</span
            >{/if}
        {/each}
      </div>
      {#if envsProblem}<p id="settings-envs-error" class="field-error">
          {envsProblem}
        </p>{/if}
    </div>
    {#if environments.length > 1}
      <span class="field-label" aria-hidden="true">Umgebung</span>
      <Segmented
        name="settings-default-env"
        label="Standard-Umgebung"
        items={environments}
        value={settings.defaultEnv}
        onChange={(value) => (draft.value.defaultEnv = value)}
      />
    {/if}
    <span class="field-label" aria-hidden="true">Agent</span>
    <Segmented
      name="settings-default-agent"
      label="Standard-Agent"
      items={agents}
      value={settings.defaultAgent}
      onChange={(value) => (draft.value.defaultAgent = value)}
    />
    <span class="field-label" aria-hidden="true">Terminal</span>
    <Segmented
      name="settings-terminal"
      label="Terminal"
      items={terminals}
      value={settings.terminal}
      onChange={(value) => (draft.value.terminal = value)}
    />
  </div>
  {#if settings.terminal === 'windowsTerminal'}
    <label
      class="switch-row"
      title="Jede Session öffnet als neuer Tab im selben Windows-Terminal-Fenster"
    >
      <input
        type="checkbox"
        class="switch-input"
        bind:checked={draft.value.sharedWindow}
      />
      <span class="switch" aria-hidden="true"></span>
      <span>Ein Fenster für alle Sessions</span>
    </label>
  {/if}
</section>
{#if draft.dockerInView}
  <section class="settings-group" aria-labelledby="docker-heading">
    <h3 id="docker-heading" class="heading-icon">
      <Icon name="docker" size={15} />Docker
    </h3>
    <div class="field-grid">
      {#each snapshot.catalog as entry (entry.id)}
        {@const problem = draft.problem(`docker.images.${entry.id}`)}
        <label class="label-icon" for={`docker-image-${entry.id}`}
          ><Icon name={AGENT_ICONS[entry.id]} size={13} />{entry.label}</label
        >
        <div class="field-cell">
          <input
            id={`docker-image-${entry.id}`}
            class="text-field mono"
            type="text"
            spellcheck="false"
            autocomplete="off"
            title={`Image für ${entry.label}`}
            aria-invalid={problem ? 'true' : undefined}
            aria-describedby={problem
              ? `docker-image-${entry.id}-error`
              : undefined}
            bind:value={draft.value.docker.images[entry.id]}
          />
          {#if problem}<p
              id={`docker-image-${entry.id}-error`}
              class="field-error"
            >
              {problem}
            </p>{/if}
        </div>
      {/each}
      <span class="field-label port-defaults-label" aria-hidden="true"
        >Standard-Ports</span
      >
      <PortPicker
        id="settings-ports"
        label="Standard-Ports"
        presets={snapshot.portPresets}
        value={settings.docker.ports}
        onChange={(next) => (draft.value.docker.ports = next)}
      />
    </div>
    <label
      class="switch-row"
      title="Übergibt das Token von „gh auth token“ als GH_TOKEN an den Container"
    >
      <input
        type="checkbox"
        class="switch-input"
        bind:checked={draft.value.docker.ghToken}
      />
      <span class="switch" aria-hidden="true"></span>
      <span>GitHub-Token weitergeben</span>
    </label>
  </section>
{/if}
