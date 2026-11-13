<script lang="ts">
  import { untrack } from 'svelte';
  import Composer from './Composer.svelte';
  import EmptyState from './EmptyState.svelte';
  import RecentList from './RecentList.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import { Composer as ComposerState } from './composerState.svelte';
  import { catalogEntry, sentOptions } from './agents';
  import { RestartGate } from './rowStart';
  import type { Panel } from './panel.svelte';
  import type { AgentId, RecentSession, Snapshot } from './types';

  let {
    snapshot,
    panel,
    onInstall,
    onEdit,
    onConnectGithub,
  }: {
    snapshot: Snapshot | null;
    panel: Panel;
    /** Opens Einstellungen → Agenten at that agent. */
    onInstall: (agent: AgentId) => void;
    /** „Bearbeiten …“ of a „Zuletzt“ row. */
    onEdit: (recent: RecentSession) => void;
    /** Opens Einstellungen → GitHub. */
    onConnectGithub: () => void;
  } = $props();
  const loading = $derived(!snapshot);
  const composer = new ComposerState();
  /** Shared by the rows, so a re-rendered row keeps its cooldown. */
  const gate = new RestartGate();
  let section = $state<HTMLElement>();

  // Only the snapshot drives the sync; the composer's own state never does.
  $effect(() => {
    const value = snapshot;
    if (value) untrack(() => composer.sync(value));
  });

  /** „In Composer übernehmen“: the form at the top, ready for Enter. */
  function adopt(recent: RecentSession) {
    composer.adopt(recent);
    section?.closest('.content')?.scrollTo({ top: 0 });
    section?.querySelector<HTMLElement>('.composer-start')?.focus();
  }
  /**
   * A row's start: exactly the options and ports it shows, as a new session,
   * once per press (`rowStart.ts`). A session started from a repository
   * sends its folder with the repository: Rust reuses the folder while it
   * exists and clones into the current Klon-Ordner only when it is gone, so a
   * clone that failed is tried again (#40). Its profile goes as stored; one
   * removed since is refused natively with a German message.
   */
  function restart(recent: RecentSession) {
    const entry = snapshot ? catalogEntry(snapshot, recent.agent) : undefined;
    gate.run(recent.id, () =>
      panel.launch({
        agent: recent.agent,
        env: recent.env,
        folder: recent.folder,
        ...(recent.repo && { repo: { ...recent.repo } }),
        options: sentOptions(entry, recent.options),
        session: 'new',
        ports: [...recent.ports],
        ...(recent.profile && { profile: recent.profile }),
      }),
    );
  }
</script>

<div class="dashboard" aria-busy={loading ? 'true' : undefined}>
  <!-- The composer sits right under the app bar, like RepoWatch's search
       field; its section is named for assistive tech only. -->
  <section
    class="dashboard-section"
    aria-label="Neue Session"
    bind:this={section}
  >
    {#if snapshot && composer.ready}
      <Composer
        {snapshot}
        {composer}
        launching={panel.launching}
        onLaunch={(request) => panel.launch(request)}
        {onInstall}
        onPickFolder={(initial) => panel.pickFolder(initial)}
        {onConnectGithub}
        onFieldFocus={() => panel.github.refreshIfStale()}
      />
    {:else}
      <div class="composer">
        <EmptyState icon="robot" iconSize={20} title="Lädt …" />
      </div>
    {/if}
  </section>
  <section class="dashboard-section" aria-labelledby="section-recent">
    <SectionHeader
      id="section-recent"
      title="Zuletzt"
      icon="clock"
      count={snapshot?.recents.length}
      unit={['Session', 'Sessions']}
    />
    {#if snapshot}
      <RecentList
        {snapshot}
        launching={panel.launching}
        onRestart={restart}
        onAdopt={adopt}
        {onEdit}
        onOpenFolder={(path) => void panel.openFolder(path)}
        onCopyFolder={(path) => void panel.copyFolder(path)}
        onRemove={(recent) => panel.removeRecent(recent)}
        {onInstall}
      />
    {/if}
  </section>
</div>
