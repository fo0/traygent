<script lang="ts">
  import RecentRow from './RecentRow.svelte';
  import { minuteClock } from './clock';
  import type { AgentId, RecentSession, Snapshot } from './types';

  // „Zuletzt“: one single-line row per (folder, agent, env), newest first.
  let {
    snapshot,
    launching,
    onRestart,
    onAdopt,
    onEdit,
    onOpenFolder,
    onRemove,
    onInstall,
  }: {
    snapshot: Snapshot;
    /** A start runs; no row starts meanwhile. */
    launching: boolean;
    onRestart: (recent: RecentSession) => void;
    onAdopt: (recent: RecentSession) => void;
    onEdit: (recent: RecentSession) => void;
    onOpenFolder: (path: string) => void;
    onRemove: (recent: RecentSession) => Promise<void>;
    onInstall: (agent: AgentId) => void;
  } = $props();
</script>

{#if snapshot.recents.length}
  <ul class="recent-list">
    {#each snapshot.recents as recent (recent.id)}
      <RecentRow
        {recent}
        {snapshot}
        now={$minuteClock}
        {launching}
        {onRestart}
        {onAdopt}
        {onEdit}
        {onOpenFolder}
        {onRemove}
        {onInstall}
      />
    {/each}
  </ul>
{:else}
  <p class="section-empty">Noch keine Sessions</p>
{/if}
