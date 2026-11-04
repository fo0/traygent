<script lang="ts">
  import { onDestroy, tick, untrack } from 'svelte';
  import GithubConnect from './GithubConnect.svelte';
  import Icon from './Icon.svelte';
  import InlineConfirm from './InlineConfirm.svelte';
  import { dateTime, errorMessage, plural } from './format';
  import type { GithubActions } from './githubActions.svelte';
  import type { SettingsDraft } from './settingsDraft.svelte';
  import type { GithubSource, Snapshot } from './types';

  // „GitHub“: the one account and its listing (immediate actions) and the
  // clone root (staged until „Speichern“).
  let {
    snapshot,
    draft,
    actions,
  }: { snapshot: Snapshot; draft: SettingsDraft; actions: GithubActions } =
    $props();
  const SOURCES: Record<GithubSource, string> = {
    token: 'Token',
    gh: 'gh CLI',
  };
  const github = $derived(snapshot.github);
  const account = $derived(github.account);
  const listing = $derived(github.loading || actions.refreshing);
  const problem = $derived(draft.problem('github.cloneRoot'));
  let confirming = $state(false);
  let disconnecting = $state(false);
  let error = $state('');
  let section = $state<HTMLElement>();
  let trigger = $state<HTMLButtonElement>();
  let alive = true;
  onDestroy(() => (alive = false));

  // Connecting or disconnecting swaps the view under the focus: it moves to
  // the new view's first control instead of dropping to the page.
  let connected = untrack(() => !!snapshot.github.account);
  $effect(() => {
    const now = !!account;
    if (now === connected) return;
    connected = now;
    confirming = false;
    error = '';
    const focused = document.activeElement;
    if (focused && focused !== document.body && !section?.contains(focused))
      return;
    void tick().then(() =>
      section?.querySelector<HTMLElement>('button, input')?.focus(),
    );
  });

  async function disconnect() {
    disconnecting = true;
    error = '';
    try {
      await actions.disconnect();
    } catch (cause) {
      if (alive) error = errorMessage(cause);
    } finally {
      if (alive) disconnecting = false;
    }
  }
  /** The confirmation closes under the focus: it returns to „Trennen“. */
  function cancelDisconnect() {
    confirming = false;
    void tick().then(() => trigger?.focus());
  }
  async function refresh() {
    if (listing) return;
    error = '';
    try {
      await actions.refresh();
    } catch (cause) {
      if (alive) error = errorMessage(cause);
    }
  }
</script>

<section
  class="settings-group"
  aria-labelledby="github-account-heading"
  bind:this={section}
>
  <h3 id="github-account-heading">Konto</h3>
  {#if account}
    <div class="github-account">
      <span class="github-login"
        ><Icon name="github" size={14} /><span
          >{account.login} · {SOURCES[account.source]}</span
        ></span
      >
      <span
        class="github-count"
        title={github.listedAt
          ? `Stand ${dateTime(github.listedAt)}`
          : undefined}
        >{listing
          ? 'Lädt …'
          : plural(github.repos.length, 'Repo', 'Repos')}</span
      >
      <button
        type="button"
        class="text-button"
        aria-disabled={listing ? 'true' : undefined}
        onclick={() => void refresh()}
        ><Icon name="refresh" size={13} />Aktualisieren</button
      >
      <button
        type="button"
        class="text-button danger-text"
        bind:this={trigger}
        aria-expanded={confirming}
        onclick={() => (confirming = true)}>Trennen</button
      >
    </div>
    {#if confirming}<InlineConfirm
        id="github-disconnect"
        title={`${account.login} trennen?`}
        text="Das Token wird aus dem Windows-Tresor entfernt."
        confirmLabel="Trennen"
        busyLabel="Wird getrennt …"
        busy={disconnecting}
        onConfirm={() => void disconnect()}
        onCancel={cancelDisconnect}
      />{/if}
    {#if github.error}<p class="github-warning" role="alert">
        <Icon name="warning" size={13} />{github.error}
      </p>{/if}
  {:else}
    <GithubConnect {actions} />
  {/if}
  {#if error}<p class="field-error" role="alert">{error}</p>{/if}
</section>
<div class="settings-group field-grid">
  <label for="github-clone-root">Klon-Ordner</label>
  <div class="field-cell">
    <input
      id="github-clone-root"
      class="text-field mono"
      type="text"
      spellcheck="false"
      autocomplete="off"
      placeholder={github.defaultCloneRoot || 'Standard'}
      title="Repositories landen in <Klon-Ordner>\<owner>\<name>"
      aria-invalid={problem ? 'true' : undefined}
      aria-describedby={problem ? 'github-clone-root-error' : undefined}
      bind:value={draft.value.github.cloneRoot}
    />
    {#if problem}<p id="github-clone-root-error" class="field-error">
        {problem}
      </p>{/if}
  </div>
</div>
