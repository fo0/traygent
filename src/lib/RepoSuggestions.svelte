<script lang="ts">
  import Icon from './Icon.svelte';
  import { repoLabel } from './target';
  import type { RepoSummary } from './types';

  // The folder field's listbox: matching repositories, or without an account
  // the one entry that leads to Settings → GitHub. Keys stay on the field
  // (combobox pattern); a press keeps the field's focus, a click picks.
  let {
    id,
    repos,
    connect,
    active,
    onPick,
    onHover,
  }: {
    id: string;
    repos: readonly RepoSummary[];
    /** No account: the list holds „GitHub verbinden …“ only. */
    connect: boolean;
    active: number;
    onPick: (index: number) => void;
    onHover: (index: number) => void;
  } = $props();

  const keepFocus = (event: MouseEvent) => event.preventDefault();
  const title = (repo: RepoSummary) =>
    [repo.private && 'Privat', repo.description].filter(Boolean).join(' · ') ||
    undefined;
</script>

<ul class="menu repo-suggestions" role="listbox" {id} aria-label="Repositories">
  {#if connect}
    <li
      id={`${id}-0`}
      role="option"
      tabindex="-1"
      class="menu-item"
      aria-selected={active === 0}
      onmousedown={keepFocus}
      onclick={() => onPick(0)}
      onkeydown={() => {}}
    >
      <Icon name="github" size={14} />GitHub verbinden …
    </li>
  {:else}
    {#each repos as repo, index (repoLabel(repo))}
      <li
        id={`${id}-${index}`}
        role="option"
        tabindex="-1"
        class="menu-item"
        aria-selected={active === index}
        aria-label={`${repoLabel(repo)}${repo.private ? ' · privat' : ''}`}
        title={title(repo)}
        onmousedown={keepFocus}
        onmousemove={() => onHover(index)}
        onclick={() => onPick(index)}
        onkeydown={() => {}}
      >
        <Icon name={repo.private ? 'lock' : 'repo'} size={14} /><span
          class="repo-label"
          ><span class="repo-owner">{repo.owner}/</span>{repo.name}</span
        >
      </li>
    {/each}
  {/if}
</ul>
