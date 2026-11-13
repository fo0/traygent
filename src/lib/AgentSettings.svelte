<script lang="ts">
  import { onDestroy } from 'svelte';
  import AgentDefaults from './AgentDefaults.svelte';
  import CopyField from './CopyField.svelte';
  import Icon from './Icon.svelte';
  import ProfileSettings from './ProfileSettings.svelte';
  import Segmented, { type Segment } from './Segmented.svelte';
  import {
    AGENT_ICONS,
    STATE_LABELS,
    agentStatus,
    catalogEntry,
    dockerLine,
    envStatus,
  } from './agents';
  import { errorMessage } from './format';
  import { installNote } from './panel.svelte';
  import type { ProfileActions } from './profileActions.svelte';
  import type { SettingsDraft } from './settingsDraft.svelte';
  import type { AgentId, ProbeState, Snapshot } from './types';

  // „Agenten“: one agent at a time — what was detected, how to install it,
  // and its staged defaults.
  let {
    snapshot,
    draft,
    agent = $bindable(),
    demo,
    profiles,
    onInstall,
  }: {
    snapshot: Snapshot;
    draft: SettingsDraft;
    agent: AgentId;
    demo: boolean;
    /** „Profile“: sign-in and sync, both immediate. */
    profiles: ProfileActions;
    /**
     * Opens the install terminal; rejects on failure. Resolves true when the
     * dialog closes with it (the composer's install flow), so no note is due.
     */
    onInstall: (agent: AgentId) => Promise<boolean>;
  } = $props();

  const entry = $derived(catalogEntry(snapshot, agent));
  const local = $derived(agentStatus(snapshot, agent, 'local'));
  const docker = $derived.by(() => {
    const engine = envStatus(snapshot, 'docker');
    const status = agentStatus(snapshot, agent, 'docker');
    if (engine?.state !== 'ready')
      return { ...dockerLine(engine), detail: engine?.detail };
    const state: ProbeState = status?.state ?? 'unknown';
    return { text: STATE_LABELS[state], state, detail: status?.detail };
  });
  const localText = $derived(
    local?.state === 'ready'
      ? (local.version ?? 'bereit')
      : local?.state === 'missing'
        ? 'Nicht installiert'
        : STATE_LABELS[local?.state ?? 'unknown'],
  );
  /** One line each; the tooltip holds everything, also what is cut off. */
  const localTitle = $derived(
    [
      localText,
      local?.detail,
      local?.path,
      entry?.install.update && `Update: ${entry.install.update}`,
    ]
      .filter(Boolean)
      .join(' · '),
  );
  const dockerTitle = $derived(
    [docker.text, docker.detail].filter(Boolean).join(' · '),
  );
  const agents = $derived<Segment<AgentId>[]>(
    snapshot.catalog.map((item) => {
      const state = agentStatus(snapshot, item.id, 'local')?.state;
      return {
        id: item.id,
        label: item.label,
        icon: AGENT_ICONS[item.id],
        dot: state ?? 'unknown',
        hint: `Lokal ${STATE_LABELS[state ?? 'unknown']}`,
      };
    }),
  );

  let installing = $state(false);
  let note = $state('');
  let failure = $state('');
  let timer: ReturnType<typeof setTimeout> | undefined;
  onDestroy(() => clearTimeout(timer));
  async function install() {
    if (installing) return;
    installing = true;
    note = failure = '';
    clearTimeout(timer);
    try {
      if (await onInstall(agent)) return;
      note = installNote(demo);
      timer = setTimeout(() => (note = ''), 3000);
    } catch (cause) {
      failure = errorMessage(cause);
    } finally {
      installing = false;
    }
  }
</script>

<Segmented
  name="settings-agent"
  label="Agent"
  items={agents}
  value={agent}
  onChange={(id) => {
    agent = id;
    note = failure = '';
  }}
/>
{#if entry}
  <section class="settings-group" aria-label={entry.label}>
    <dl class="status-lines">
      <div>
        <dt><Icon name="monitor" size={13} />Lokal</dt>
        <dd data-state={local?.state ?? 'unknown'} title={localTitle}>
          {localText}{#if local?.detail}<span class="status-detail"
              >{` · ${local.detail}`}</span
            >{/if}
        </dd>
      </div>
      {#if draft.dockerInView}<div>
          <dt><Icon name="docker" size={13} />Docker</dt>
          <dd data-state={docker.state} title={dockerTitle}>
            {docker.text}{#if docker.detail}<span class="status-detail"
                >{` · ${docker.detail}`}</span
              >{/if}
          </dd>
        </div>{/if}
    </dl>
  </section>
  <section class="settings-group" aria-labelledby="install-heading">
    <h3 id="install-heading">
      Installation<span
        class="info-tip"
        role="img"
        aria-label={`Doku: ${entry.install.docsUrl}`}
        title={entry.install.docsUrl}><Icon name="info" size={13} /></span
      >
    </h3>
    <div class="install-row">
      <CopyField value={entry.install.command} label="Installationsbefehl" />
      <button
        type="button"
        class="secondary-button compact"
        aria-disabled={installing ? 'true' : undefined}
        title="Öffnet ein Terminal mit dem Befehl"
        onclick={() => void install()}
        >{installing ? 'Öffnet …' : 'Installieren'}</button
      >
    </div>
    {#if entry.install.requires}<p class="field-help">
        {entry.install.requires}
      </p>{/if}
    {#if note}<p class="install-note" role="status">
        <Icon name="check" size={13} />{note}
      </p>{/if}
    {#if failure}<div class="alert danger" role="alert">
        <Icon name="warning" size={16} /><span>{failure}</span>
      </div>{/if}
  </section>
  <AgentDefaults {entry} {draft} />
  {#if entry.profiles}<ProfileSettings
      {entry}
      {snapshot}
      {draft}
      {demo}
      actions={profiles}
    />{/if}
{/if}
