<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import AgentSettings from './AgentSettings.svelte';
  import AgentUpdates from './AgentUpdates.svelte';
  import EnvSettings from './EnvSettings.svelte';
  import GithubSettings from './GithubSettings.svelte';
  import Icon from './Icon.svelte';
  import Modal from './Modal.svelte';
  import StartupSettings from './StartupSettings.svelte';
  import ThemeSetting from './ThemeSetting.svelte';
  import UpdatePanel from './UpdatePanel.svelte';
  import { Autostart } from './autostart.svelte';
  import { errorMessage } from './format';
  import type { GithubActions } from './githubActions.svelte';
  import { SettingsDraft } from './settingsDraft.svelte';
  import { SETTINGS_TABS, type SettingsTab } from './settingsTabs';
  import { appliedTheme, applyTheme, rememberTheme } from './theme';
  import type { AgentId, AppSettings, Snapshot } from './types';
  import type { Updater } from './updater.svelte';

  let {
    native,
    demo,
    updater,
    appVersion,
    snapshot,
    initialTab = 'agents',
    initialAgent = null,
    closeAfterInstall = false,
    returnFocus,
    github,
    onSaveSettings,
    onInstallAgent,
    onUpdateAgents,
    onInstalled,
    onClose,
  }: {
    native: boolean;
    demo: boolean;
    updater: Updater;
    appVersion: string;
    /** Catalog and probes; its settings are the staged baseline. */
    snapshot: Snapshot;
    initialTab?: SettingsTab;
    /** The agent „Agenten“ opens at; the default agent otherwise. */
    initialAgent?: AgentId | null;
    /**
     * Opened by the composer's „Installieren …“: once the install terminal
     * opened, the flow is done and the dialog closes (UX rule 6), unless
     * something is staged or the agent's local status carries a `detail`.
     */
    closeAfterInstall?: boolean;
    /** Settings → GitHub: account actions that apply at once. */
    github: GithubActions;
    /** Saves the whole settings object; rejects with the German message. */
    onSaveSettings: (settings: AppSettings) => Promise<Snapshot>;
    /** An immediate action, not staged: opens the install terminal. */
    onInstallAgent: (agent: AgentId) => Promise<void>;
    /** An immediate action, not staged: starts an agent CLI update run. */
    onUpdateAgents: () => Promise<void>;
    /** The install flow closed the dialog: the panel says what comes next. */
    onInstalled?: () => void;
    returnFocus?: (() => HTMLElement | null | undefined) | undefined;
    /** Closes without applying: „Abbrechen“, Escape and the close button. */
    onClose: () => void;
  } = $props();

  // Settings are staged and applied together by „Speichern“; updates act at
  // once. A new tab adds its entry to SETTINGS_TABS, its panel below and its
  // staged changes to `changed` and `save()`.
  let tab = $state<SettingsTab>(untrack(() => initialTab));
  let agent = $state<AgentId>(
    untrack(() => initialAgent ?? snapshot.settings.defaultAgent),
  );
  // Later snapshots (a finished detection) refresh the probes only.
  const draft = new SettingsDraft(untrack(() => snapshot.settings));
  let error = $state('');
  let saving = $state(false);
  let alive = true;
  // main.ts applied the saved theme; the choice only previews until saved.
  let savedTheme = $state(appliedTheme());
  /** Plain copy for the teardown, which reads state as it was before close. */
  let keptTheme = untrack(() => savedTheme);
  let theme = $state(untrack(() => savedTheme));
  const autostart = new Autostart(untrack(() => native));
  onDestroy(() => {
    alive = false;
    autostart.dispose();
    // Closing without „Speichern“ drops the preview.
    if (appliedTheme() !== keptTheme) applyTheme(keptTheme);
  });
  const changed = $derived({
    settings: draft.dirty,
    theme: theme !== savedTheme,
    autostart: autostart.pending !== null,
  });
  const dirty = $derived(Object.values(changed).some(Boolean));
  const saveBlocked = $derived(!dirty || saving || draft.invalid);
  /**
   * Tab states: a dot whose text is part of the tab's name. An invalid value
   * marks its tab with `tone: 'danger'`, so a blocked „Speichern“ is
   * explained from every tab.
   */
  const badges = $derived<
    Partial<Record<SettingsTab, { text: string; tone?: 'danger' }>>
  >({
    ...(draft.invalidIn('agents') && {
      agents: { text: 'Ungültige Eingabe', tone: 'danger' },
    }),
    ...((draft.invalidIn('docker') || draft.invalidIn('environments')) && {
      environment: { text: 'Ungültige Eingabe', tone: 'danger' },
    }),
    ...(draft.invalidIn('github') && {
      github: { text: 'Ungültige Eingabe', tone: 'danger' },
    }),
    ...(updater.info && { updates: { text: 'Update verfügbar' } }),
  });

  /**
   * Applies every staged change in turn and closes. A change that applied
   * becomes the new baseline; the first failure keeps the dialog open with
   * its error, and the remaining changes stay staged.
   */
  async function save() {
    if (saveBlocked) return;
    const pending = changed;
    saving = true;
    error = '';
    try {
      if (pending.settings) {
        const next = await onSaveSettings(draft.current());
        if (alive) draft.commit(next.settings);
      }
      if (pending.theme) {
        const value = theme;
        savedTheme = keptTheme = value;
        if (!rememberTheme(value))
          throw new Error(
            'Darstellung ließ sich nicht speichern und gilt bis zum nächsten Start.',
          );
      }
      if (pending.autostart) await autostart.apply();
      if (alive) onClose();
    } catch (cause) {
      if (alive) error = errorMessage(cause);
    } finally {
      if (alive) saving = false;
    }
  }

  /**
   * Resolves true when the install flow closed the dialog. A local status
   * with a `detail` (e.g. an OpenCode 1 to remove first) keeps it open, so
   * the reason the install may fail stays in view.
   */
  async function install(id: AgentId) {
    await onInstallAgent(id);
    if (!alive || !closeAfterInstall || dirty) return false;
    const local = snapshot.agents.find(
      (status) => status.agent === id && status.env === 'local',
    );
    if (local?.detail) return false;
    onInstalled?.();
    onClose();
    return true;
  }

  function tabKey(event: KeyboardEvent & { currentTarget: HTMLElement }) {
    const direction =
      event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
    if (!direction && event.key !== 'Home' && event.key !== 'End') return;
    event.preventDefault();
    const count = SETTINGS_TABS.length;
    const index = SETTINGS_TABS.findIndex((item) => item.id === tab);
    const next =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? count - 1
          : (index + direction + count) % count;
    const target = SETTINGS_TABS[next];
    if (!target) return;
    tab = target.id;
    event.currentTarget
      .querySelectorAll<HTMLButtonElement>('[role=tab]')
      [next]?.focus();
  }
</script>

<Modal
  title="Einstellungen"
  eyebrow={`TRAYGENT · ${demo ? 'DEMO' : 'LOKAL'}`}
  wide
  {returnFocus}
  closeDisabled={saving}
  onClose={() => !saving && onClose()}
>
  <div
    class="settings-tabs"
    role="tablist"
    tabindex="-1"
    aria-label="Einstellungsbereiche"
    onkeydown={tabKey}
  >
    {#each SETTINGS_TABS as item (item.id)}
      <button
        id={`settings-tab-${item.id}`}
        role="tab"
        aria-selected={tab === item.id}
        aria-controls="settings-panel"
        tabindex={tab === item.id ? 0 : -1}
        onclick={() => (tab = item.id)}
        ><Icon
          name={item.icon}
          size={14}
        />{item.label}{#if badges[item.id]}<span
            class="tab-update-dot"
            data-tone={badges[item.id]?.tone}
            role="img"
            aria-label={badges[item.id]?.text}
            title={badges[item.id]?.text}
          ></span>{/if}</button
      >
    {/each}
  </div>
  <div
    class="modal-body"
    id="settings-panel"
    role="tabpanel"
    aria-labelledby={`settings-tab-${tab}`}
    tabindex="0"
  >
    {#if tab === 'agents'}
      <AgentSettings {snapshot} {draft} bind:agent {demo} onInstall={install} />
    {:else if tab === 'environment'}
      <EnvSettings {snapshot} {draft} />
    {:else if tab === 'github'}
      <GithubSettings {snapshot} {draft} actions={github} />
    {:else if tab === 'app'}
      <ThemeSetting bind:value={theme} />
      <StartupSettings {native} {autostart} />
    {:else}
      <UpdatePanel
        currentVersion={appVersion}
        {native}
        info={updater.info}
        phase={updater.phase}
        progress={updater.progress}
        error={updater.message}
        checkedAt={updater.checkedAt}
        onCheck={() => updater.check()}
        onInstall={updater.install}
      />
      <AgentUpdates {snapshot} {draft} onUpdate={onUpdateAgents} />
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
      type="button"
      class="primary-button"
      aria-disabled={saveBlocked ? 'true' : undefined}
      title={draft.invalid
        ? 'Ungültige Eingabe'
        : dirty || saving
          ? undefined
          : 'Keine Änderungen'}
      onclick={() => void save()}
      >{saving ? 'Wird gespeichert …' : 'Speichern'}</button
    >
  </div>
</Modal>
