<script lang="ts">
  import FolderField from './FolderField.svelte';
  import Icon from './Icon.svelte';
  import OptionChips from './OptionChips.svelte';
  import Segmented, { type Segment } from './Segmented.svelte';
  import {
    ENVIRONMENTS,
    AGENT_ICONS,
    SOON,
    STATE_LABELS,
    agentStatus,
    catalogEntry,
    dockerLine,
    envStatus,
    shimStart,
    startState,
  } from './agents';
  import type { Composer } from './composerState.svelte';
  import { recentFolders } from './recents';
  import { targetProblem } from './target';
  import type {
    AgentId,
    AgentStatus,
    EnvId,
    EnvStatus,
    LaunchRequest,
    Snapshot,
  } from './types';
  import {
    launchPortsProblem,
    modelProblem,
    optionsProblem,
  } from './validation';

  let {
    snapshot,
    composer,
    launching,
    onLaunch,
    onInstall,
    onPickFolder,
    onConnectGithub,
    onFieldFocus,
  }: {
    snapshot: Snapshot;
    composer: Composer;
    launching: boolean;
    /** Resolves true when the terminal opened. */
    onLaunch: (request: LaunchRequest) => Promise<boolean>;
    /** Opens Einstellungen → Agenten at the missing agent. */
    onInstall: (agent: AgentId) => void;
    onPickFolder: (initial: string | null) => Promise<string | null>;
    /** „GitHub verbinden …“: Settings → GitHub. */
    onConnectGithub: () => void;
    /** The folder field got focus: a stale listing is read again. */
    onFieldFocus: () => void;
  } = $props();

  let form = $state<HTMLFormElement>();
  const entry = $derived(catalogEntry(snapshot, composer.agent));
  const target = $derived(startState(snapshot, composer));
  const invalidModel = $derived(
    modelProblem(composer.options.model, composer.agent),
  );
  /** Why „Starten“ cannot run now; empty when it can. */
  const blocked = $derived(
    target.kind === 'blocked'
      ? target.reason
      : targetProblem(composer.target) ||
          optionsProblem(
            composer,
            shimStart(snapshot, composer.agent, composer.env),
          ) ||
          launchPortsProblem(composer.request()),
  );

  function agentHint(label: string, status: AgentStatus | undefined) {
    const state = STATE_LABELS[status?.state ?? 'unknown'];
    const version = status?.version ? ` ${status.version}` : '';
    const detail = status?.detail ? ` · ${status.detail}` : '';
    return `${label}${version} · ${state}${detail}`;
  }
  const agents = $derived<Segment<AgentId>[]>(
    snapshot.catalog.map((item) => {
      const status = agentStatus(snapshot, item.id, composer.env);
      return {
        id: item.id,
        label: item.label,
        icon: AGENT_ICONS[item.id],
        dot: status?.state ?? 'unknown',
        hint: agentHint(item.label, status),
      };
    }),
  );
  /** Only WSL is „bald“; a Docker that is down stays selectable and says why. */
  function envHint(env: EnvId, status: EnvStatus | undefined) {
    if (env === 'wsl') return SOON;
    if (env !== 'docker') return undefined;
    const line = dockerLine(status).text;
    return status?.detail ? `${line} · ${status.detail}` : line;
  }
  /** The shown environments; a single one needs no row. */
  const environments = $derived<Segment<EnvId>[]>(
    ENVIRONMENTS.filter((env) => composer.environments.includes(env.id)).map(
      (env) => ({
        id: env.id,
        label: env.label,
        icon: env.icon,
        disabled: env.id === 'wsl',
        hint: envHint(env.id, envStatus(snapshot, env.id)),
      }),
    ),
  );
  const startTitle = $derived(
    target.kind === 'install'
      ? `${entry?.label ?? composer.agent} ist lokal nicht installiert`
      : blocked || 'Starten (Enter)',
  );

  /** Enter anywhere in the composer, the button itself, or a menu-less click. */
  function start() {
    if (target.kind === 'install') return onInstall(composer.agent);
    if (blocked || launching) return;
    void onLaunch(composer.request()).then((ok) => ok && composer.launched());
  }
  async function browse() {
    const field = composer.target;
    const picked = await onPickFolder(
      field.kind === 'folder' ? field.folder : null,
    );
    if (picked) composer.folder = picked;
  }
</script>

<form
  class="composer"
  bind:this={form}
  novalidate
  onsubmit={(event) => {
    event.preventDefault();
    start();
  }}
>
  <Segmented
    name="composer-agent"
    label="Agent"
    items={agents}
    value={composer.agent}
    onChange={(id) => composer.selectAgent(id)}
    landing
    prominent
    collapse
  />
  <FolderField
    bind:value={composer.folder}
    recents={recentFolders(snapshot.recents)}
    repos={snapshot.github.repos}
    connected={!!snapshot.github.account}
    onBrowse={() => void browse()}
    onConnect={onConnectGithub}
    onFocus={onFieldFocus}
  />
  {#if environments.length > 1}<Segmented
      name="composer-env"
      label="Umgebung"
      items={environments}
      value={composer.env}
      onChange={(id) => composer.selectEnv(id)}
    />{/if}
  <div class="composer-actions">
    {#if entry}<OptionChips
        {entry}
        {composer}
        presets={snapshot.portPresets}
        {invalidModel}
        onEnter={() => form?.requestSubmit()}
      />{/if}
    <!-- Off via aria-disabled: it keeps its tooltip and its focus. -->
    <button
      type="submit"
      class="primary-button composer-start"
      aria-disabled={(target.kind !== 'install' && !!blocked) || launching
        ? 'true'
        : undefined}
      aria-keyshortcuts="Enter"
      title={startTitle}
      >{#if target.kind === 'install'}Installieren …{:else}<Icon
          name="play"
          size={12}
        />{launching ? 'Startet …' : 'Starten'}{/if}</button
    >
  </div>
</form>
