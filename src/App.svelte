<script lang="ts">
  import { onMount } from 'svelte';
  import AppBar from './lib/AppBar.svelte';
  import AppFooter from './lib/AppFooter.svelte';
  import Dashboard from './lib/Dashboard.svelte';
  import RecentEditDialog from './lib/RecentEditDialog.svelte';
  import SettingsDialog from './lib/SettingsDialog.svelte';
  import StatusBanners from './lib/StatusBanners.svelte';
  import appPackage from '../package.json';
  import { bridge, native, observeSnapshots } from './lib/bridge';
  import { Modals } from './lib/modals.svelte';
  import { Panel } from './lib/panel.svelte';
  import { createShortcuts } from './lib/shortcuts';
  import { Updater } from './lib/updater.svelte';

  const appVersion = appPackage.version;
  const panel = new Panel({ bridge, native, observe: observeSnapshots });
  const updater = new Updater(native);
  const modals = new Modals();
  const snapshot = $derived(panel.snapshot);
  const demo = $derived(panel.demo);

  /** The brand leads home: the dashboard from its top. */
  function goHome() {
    document.querySelector('.tray .content')?.scrollTo({ top: 0 });
  }

  /** A load error offers a retry before any snapshot exists. */
  const canRefresh = $derived(!!snapshot || !!panel.error);

  onMount(() => {
    const stop = panel.load(() => {
      if (native) void updater.check(true);
    });
    const updateTimer = native
      ? setInterval(() => void updater.check(true), 6 * 60 * 60 * 1000)
      : undefined;
    return () => {
      clearInterval(updateTimer);
      stop();
    };
  });

  /** A refresh that recovers from a load error also runs the skipped startup update check. */
  async function refreshPanel() {
    const recovering = !panel.snapshot;
    const next = await panel.refresh();
    if (native && recovering && next) void updater.check(true);
  }
  // „Bearbeiten …“ of an entry that is gone (removed elsewhere) closes.
  $effect(() => {
    if (
      modals.kind === 'recent' &&
      snapshot &&
      !snapshot.recents.some((item) => item.id === modals.recentId)
    )
      modals.close();
  });
  const shortcuts = createShortcuts({
    blocked: () => modals.kind !== null,
    refresh: () => void (canRefresh && refreshPanel()),
    folder: () => {
      const field = document.querySelector<HTMLInputElement>('.folder-input');
      field?.focus();
      field?.select();
      return !!field;
    },
    // Menus handle their own Escape; the dashboard has nothing else to close.
    escape: () => false,
  });
</script>

<svelte:window onkeydown={shortcuts} />

<!-- `inert` already hides the page from assistive tech while a dialog is
     open; aria-hidden states it for tools that do not model `inert`. -->
<main
  class="desktop"
  inert={modals.kind !== null}
  aria-hidden={modals.kind !== null ? 'true' : undefined}
>
  <section class="tray" aria-label="Traygent">
    <AppBar
      {native}
      {demo}
      refreshing={panel.refreshing}
      detecting={panel.detecting}
      ready={!!snapshot}
      {canRefresh}
      onHome={goHome}
      onRefresh={() => void refreshPanel()}
      onSettings={() => modals.openSettings()}
    />

    <div class="content">
      <StatusBanners
        update={updater.info}
        {demo}
        error={panel.error}
        notice={snapshot?.notice}
        notices={panel.notices.items}
        paused={modals.kind !== null}
        onShowUpdate={() => modals.openSettings('updates')}
        onDismissNotice={(id) => panel.notices.dismiss(id)}
      />
      <Dashboard
        {snapshot}
        {panel}
        onInstall={(agent) => modals.openInstall(agent)}
        onEdit={(recent) => modals.openRecent(recent.id)}
        onConnectGithub={() => modals.openSettings('github')}
      />
    </div>

    <AppFooter
      {demo}
      checkedAt={snapshot?.checkedAt}
      docker={snapshot?.settings.environments.includes('docker')
        ? snapshot.environments.find((env) => env.env === 'docker')
        : undefined}
      refreshing={panel.refreshing}
      detecting={panel.detecting}
      {canRefresh}
      onRefresh={() => void refreshPanel()}
    />
  </section>
</main>

{#if modals.kind === 'settings' && snapshot}
  <SettingsDialog
    {native}
    {demo}
    {updater}
    {appVersion}
    {snapshot}
    initialTab={modals.settingsTab}
    initialAgent={modals.settingsAgent}
    closeAfterInstall={modals.installFlow}
    returnFocus={modals.returnFocus}
    github={panel.github}
    onSaveSettings={(settings) => panel.saveSettings(settings)}
    onInstallAgent={(agent) => panel.installAgent(agent)}
    onUpdateAgents={() => panel.updateAgents()}
    onInstalled={() => panel.announceInstall()}
    onClose={modals.close}
  />
{/if}
<!-- Follows the entry in every snapshot; one removed meanwhile closes it
     (the effect above), so the page never stays inert without a dialog. -->
{#if modals.kind === 'recent'}
  {@const recent = snapshot?.recents.find(
    (item) => item.id === modals.recentId,
  )}
  {#if snapshot && recent}
    <RecentEditDialog
      {snapshot}
      {recent}
      returnFocus={modals.returnFocus}
      onSave={(options, ports) => panel.updateRecent(recent, options, ports)}
      onClose={modals.close}
    />
  {/if}
{/if}
