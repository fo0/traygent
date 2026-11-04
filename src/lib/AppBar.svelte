<script lang="ts">
  import Icon from './Icon.svelte';

  let {
    native,
    demo,
    refreshing,
    detecting,
    ready,
    canRefresh,
    onHome,
    onRefresh,
    onSettings,
  }: {
    native: boolean;
    demo: boolean;
    refreshing: boolean;
    /** A detection runs; the icon spins, a new one may still be asked for. */
    detecting: boolean;
    /** A snapshot is loaded; settings wait for it. */
    ready: boolean;
    /** A snapshot is loaded or a load error invites a retry. */
    canRefresh: boolean;
    /** The brand: back to the overview, at its top. */
    onHome: () => void;
    onRefresh: () => void;
    onSettings: () => void;
  } = $props();
</script>

<!-- Natively the bar drags the window from its free space; its controls,
     the brand's home button among them, stay clickable. The heading keeps
     its name „Traygent“; the tooltip says where the button leads. -->
<header class="app-bar" data-tauri-drag-region={native ? true : undefined}>
  <h1 class="brand">
    <button
      type="button"
      class="brand-home"
      title="Zur Übersicht"
      onclick={() => onHome()}
      ><span class="brand-icon"><Icon name="robot" size={16} /></span><span
        class="brand-name">Traygent</span
      ></button
    >
  </h1>
  <div class="app-bar-actions">
    <button
      class:spinning={refreshing || detecting}
      class="icon-button"
      aria-label="Aktualisieren"
      aria-keyshortcuts="F5 Control+R"
      aria-busy={detecting ? 'true' : undefined}
      title={detecting
        ? 'Erkennung läuft …'
        : `${demo ? 'Demo' : 'Agenten und Docker'} neu erkennen (F5)`}
      onclick={() => onRefresh()}
      disabled={refreshing || !canRefresh}
      ><Icon name="refresh" size={17} /></button
    ><button
      class="icon-button"
      aria-label="Einstellungen"
      title="Einstellungen"
      onclick={() => onSettings()}
      disabled={!ready}><Icon name="settings" size={17} /></button
    >
  </div>
</header>
