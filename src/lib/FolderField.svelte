<script lang="ts">
  import Icon from './Icon.svelte';
  import RepoSuggestions from './RepoSuggestions.svelte';
  import { folderName, parentPath } from './agents';
  import { menu, openingFocus, pressClosedPopup, type MenuFocus } from './menu';
  import { classifyTarget, matchRepos, repoLabel } from './target';
  import type { RepoSummary } from './types';

  /** The empty field's tooltip: every kind of text it takes. */
  const FIELD_HINT =
    'Lokaler Ordner, GitHub-URL, owner/name oder Suchtext über die eigenen Repos';

  // The composer's target: a folder (typed, picked in the native dialog or
  // chosen from the recent folders) or a GitHub repository (pasted, typed as
  // owner/name or picked from the suggestions of search text).
  let {
    value = $bindable(),
    recents,
    repos,
    connected,
    onBrowse,
    onConnect,
    onFocus,
  }: {
    value: string;
    /** Distinct recent folders, newest first. */
    recents: readonly string[];
    /** The connected account's listing, newest push first. */
    repos: readonly RepoSummary[];
    connected: boolean;
    onBrowse: () => void;
    /** „GitHub verbinden …“: Settings → GitHub. */
    onConnect: () => void;
    /** The field got focus, e.g. to refresh a stale listing. */
    onFocus: () => void;
  } = $props();
  const LIST = 'folder-suggestions';
  /** Repositories the GitHub menu lists: the newest pushes. */
  const RECENT_REPOS = 8;
  /** The open menu: recent folders or recent repositories. */
  let open = $state<'folders' | 'repos' | null>(null);
  let initial = $state<MenuFocus>('first');
  let trigger = $state<HTMLButtonElement>();
  let repoTrigger = $state<HTMLButtonElement>();
  let input = $state<HTMLInputElement>();
  /** Suggestions wanted: typing opens them, Escape, a pick and blur close them. */
  let suggest = $state(false);
  let active = $state(0);

  const target = $derived(classifyTarget(value));
  const matches = $derived(
    target.kind === 'search' && connected
      ? matchRepos(repos, target.query)
      : [],
  );
  /** Search text without an account offers „GitHub verbinden …“. */
  const connect = $derived(target.kind === 'search' && !connected);
  const count = $derived(connect ? 1 : matches.length);
  const expanded = $derived(suggest && count > 0);
  const current = $derived(Math.min(active, count - 1));

  const recentRepos = $derived(repos.slice(0, RECENT_REPOS));

  function openWith(menu: 'folders' | 'repos', focus: MenuFocus) {
    // The listing may be stale: refreshed like on the field's focus.
    if (menu === 'repos' && connected) onFocus();
    initial = focus;
    open = menu;
  }
  /** A menu button: click toggles, ↓/↑ open at the first/last item. */
  function menuButton(menu: 'folders' | 'repos') {
    return {
      onclick: () => (open === menu ? (open = null) : openWith(menu, 'first')),
      onkeydown: (event: KeyboardEvent) => {
        const focus = openingFocus(event.key);
        if (!focus) return;
        event.preventDefault();
        openWith(menu, focus);
      },
    };
  }
  function choose(text: string) {
    value = text;
    open = null;
    input?.focus();
  }
  function connectFromMenu() {
    open = null;
    onConnect();
  }
  function pick(index: number) {
    suggest = false;
    if (connect) return onConnect();
    const repo = matches[index];
    if (repo) value = repoLabel(repo);
  }
  /** Combobox keys; Enter on an open list picks instead of starting. */
  function keydown(event: KeyboardEvent) {
    const step =
      event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0;
    if (step && count > 0) {
      event.preventDefault();
      if (!expanded) {
        suggest = true;
        active = step > 0 ? 0 : count - 1;
      } else active = (current + step + count) % count;
    } else if (event.key === 'Enter' && expanded) {
      event.preventDefault();
      pick(current);
    } else if (event.key === 'Escape' && expanded) {
      event.preventDefault();
      event.stopPropagation();
      suggest = false;
    }
  }
</script>

<div class="folder-field">
  {#if target.kind === 'repo'}<span
      class="folder-kind"
      role="img"
      aria-label={`GitHub-Repository ${repoLabel(target.repo)}`}
      title={`GitHub-Repository ${repoLabel(target.repo)}`}
      ><Icon name="github" size={14} /></span
    >{/if}
  <input
    bind:this={input}
    bind:value
    class="folder-input"
    type="text"
    role="combobox"
    aria-label="Ordner oder Repo"
    aria-autocomplete="list"
    aria-expanded={expanded}
    aria-controls={expanded ? LIST : undefined}
    aria-activedescendant={expanded ? `${LIST}-${current}` : undefined}
    placeholder="C:\Projekte\app oder owner/repo"
    title={value || `${FIELD_HINT} (Strg+L)`}
    aria-keyshortcuts="Control+L"
    spellcheck="false"
    autocomplete="off"
    oninput={() => {
      suggest = true;
      active = 0;
    }}
    onkeydown={keydown}
    onfocus={() => onFocus()}
    onblur={() => {
      // A press elsewhere that closes the suggestions does nothing else.
      if (expanded) pressClosedPopup();
      suggest = false;
    }}
  />
  {#if recents.length}<button
      bind:this={trigger}
      type="button"
      class="icon-button small"
      aria-label="Zuletzt verwendete Ordner"
      title="Zuletzt verwendete Ordner"
      aria-haspopup="menu"
      aria-expanded={open === 'folders'}
      {...menuButton('folders')}><Icon name="down" size={14} /></button
    >{/if}<button
    bind:this={repoTrigger}
    type="button"
    class="icon-button small"
    aria-label="GitHub-Repositories"
    title="GitHub-Repositories"
    aria-haspopup="menu"
    aria-expanded={open === 'repos'}
    {...menuButton('repos')}><Icon name="github" size={15} /></button
  ><button
    type="button"
    class="icon-button small"
    aria-label="Ordner durchsuchen"
    title="Ordner durchsuchen"
    onclick={() => onBrowse()}><Icon name="folder" size={15} /></button
  >
  {#if expanded}
    <RepoSuggestions
      id={LIST}
      repos={matches}
      {connect}
      active={current}
      onPick={pick}
      onHover={(index) => (active = index)}
    />
  {/if}
  {#if open === 'folders'}
    <div
      class="menu folder-menu"
      role="menu"
      aria-label="Zuletzt verwendete Ordner"
      use:menu={{ trigger, initial, onClose: () => (open = null) }}
    >
      {#each recents as folder (folder)}
        <button
          type="button"
          role="menuitem"
          class="menu-item"
          title={folder}
          onclick={() => choose(folder)}
          ><Icon name="folder" size={14} /><span class="menu-text"
            ><span>{folderName(folder)}</span><span class="menu-meta"
              >{parentPath(folder)}</span
            ></span
          ></button
        >
      {/each}
    </div>
  {/if}
  {#if open === 'repos'}
    <div
      class="menu folder-menu"
      role="menu"
      aria-label="GitHub-Repositories"
      use:menu={{
        trigger: repoTrigger,
        initial,
        onClose: () => (open = null),
      }}
    >
      {#if !connected}
        <button
          type="button"
          role="menuitem"
          class="menu-item"
          onclick={connectFromMenu}
          ><Icon name="github" size={14} />GitHub verbinden …</button
        >
      {:else if recentRepos.length}
        {#each recentRepos as repo (repoLabel(repo))}
          <button
            type="button"
            role="menuitem"
            class="menu-item"
            aria-label={`${repoLabel(repo)}${repo.private ? ' · privat' : ''}`}
            title={repo.description || undefined}
            onclick={() => choose(repoLabel(repo))}
            ><Icon name={repo.private ? 'lock' : 'repo'} size={14} /><span
              class="repo-label"
              ><span class="repo-owner">{repo.owner}/</span>{repo.name}</span
            ></button
          >
        {/each}
      {:else}
        <!-- No listing (yet): the way to the account instead of a dead row. -->
        <button
          type="button"
          role="menuitem"
          class="menu-item"
          title="Keine Repositories geladen"
          onclick={connectFromMenu}
          ><Icon name="github" size={14} />GitHub-Einstellungen …</button
        >
      {/if}
    </div>
  {/if}
</div>
