<script lang="ts">
  import RecentRow from './RecentRow.svelte';
  import { minuteClock } from './clock';
  import {
    PROFILE_REMOVED,
    profileLabel,
    recentProfileRemoved,
  } from './profiles';
  import { rowKeys } from './rowStart';
  import type { AgentId, RecentSession, Snapshot } from './types';

  // „Workspaces“: one single-line row per (folder, agent, env), newest first.
  let {
    snapshot,
    launching,
    onRestart,
    onAdopt,
    onEdit,
    onOpenFolder,
    onCopyFolder,
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
    onCopyFolder: (path: string) => void;
    onRemove: (recent: RecentSession) => Promise<void>;
    onInstall: (agent: AgentId) => void;
  } = $props();
</script>

{#if snapshot.recents.length}
  <!-- ↑ / ↓, Home and End move between the rows' starts (rowStart.ts). -->
  <ul class="recent-list" use:rowKeys>
    {#each snapshot.recents as recent, index (recent.id)}
      <!-- A profile removed since is named so, never by its raw id (#61). -->
      {@const profileRemoved = recentProfileRemoved(snapshot.settings, recent)}
      <RecentRow
        {recent}
        shortcut={index < 9 ? index + 1 : 0}
        {snapshot}
        now={$minuteClock}
        profile={profileRemoved
          ? PROFILE_REMOVED
          : recent.profile
            ? profileLabel(snapshot, recent.agent, recent.profile)
            : ''}
        {profileRemoved}
        {launching}
        {onRestart}
        {onAdopt}
        {onEdit}
        {onOpenFolder}
        {onCopyFolder}
        {onRemove}
        {onInstall}
      />
    {/each}
  </ul>
{:else}
  <p class="section-empty">Noch keine Workspaces</p>
{/if}
