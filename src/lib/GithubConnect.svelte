<script lang="ts">
  import { onDestroy } from 'svelte';
  import { errorMessage } from './format';
  import type { GithubActions } from './githubActions.svelte';
  import type { LocalAccounts } from './types';

  // Settings → GitHub without an account: import a local gh sign-in after an
  // explicit „Übernehmen“, or connect a personal access token. Both apply at
  // once; nothing local is read before „Lokale Anmeldung prüfen“.
  let { actions }: { actions: GithubActions } = $props();
  let phase = $state<'idle' | 'checking' | 'importing' | 'connecting'>('idle');
  let local = $state<LocalAccounts | null>(null);
  let selected = $state('');
  let token = $state('');
  let localError = $state('');
  let tokenError = $state('');
  let alive = true;
  onDestroy(() => (alive = false));
  const busy = $derived(phase !== 'idle');

  async function run(next: typeof phase, task: () => Promise<void>) {
    if (busy) return;
    phase = next;
    try {
      await task();
    } finally {
      if (alive) phase = 'idle';
    }
  }
  const check = () =>
    run('checking', async () => {
      localError = '';
      try {
        const found = await actions.localAccounts();
        if (!alive) return;
        local = found;
        selected = found.accounts.find((a) => a.importable)?.login ?? '';
      } catch (cause) {
        if (alive) localError = errorMessage(cause);
      }
    });
  const importLocal = () =>
    run('importing', async () => {
      localError = '';
      try {
        if (selected) await actions.importLocal(selected);
      } catch (cause) {
        if (alive) localError = errorMessage(cause);
      }
    });
  const connectToken = () =>
    run('connecting', async () => {
      if (!token.trim()) return;
      tokenError = '';
      try {
        await actions.connectToken(token.trim());
        if (alive) token = '';
      } catch (cause) {
        if (alive) tokenError = errorMessage(cause);
      }
    });
</script>

<div class="github-local">
  <button
    type="button"
    class="secondary-button compact"
    aria-disabled={busy ? 'true' : undefined}
    title="Anmeldungen der GitHub CLI (gh) auf diesem PC"
    onclick={() => void check()}
    >{phase === 'checking'
      ? 'Wird geprüft …'
      : 'Lokale Anmeldung prüfen'}</button
  >
  {#if local}
    {#if local.message}<p class="github-note">{local.message}</p>{/if}
    {#if local.accounts.length}
      <div
        class="local-accounts"
        role="radiogroup"
        aria-label="Anmeldungen der GitHub CLI"
      >
        {#each local.accounts as account (account.login)}
          <label class="local-account" class:off={!account.importable}>
            <input
              type="radio"
              name="github-local"
              value={account.login}
              disabled={!account.importable || busy}
              bind:group={selected}
            /><span>{account.login}</span>{#if !account.importable}<span
                class="local-account-meta">nicht übernehmbar</span
              >{/if}
          </label>
        {/each}
      </div>
    {/if}
    {#if selected}
      <div class="github-import">
        <p class="github-note" id="github-consent">
          Das Token von {selected} wird aus der GitHub CLI übernommen und im Windows-Tresor
          gespeichert.
        </p>
        <button
          type="button"
          class="secondary-button compact"
          aria-describedby="github-consent"
          aria-disabled={busy ? 'true' : undefined}
          onclick={() => void importLocal()}
          >{phase === 'importing' ? 'Wird übernommen …' : 'Übernehmen'}</button
        >
      </div>
    {/if}
  {/if}
  {#if localError}<p class="field-error" role="alert">{localError}</p>{/if}
</div>
<div class="field-grid github-token">
  <label for="github-token">Token</label>
  <div class="field-cell">
    <div class="install-row">
      <input
        id="github-token"
        class="text-field mono"
        type="password"
        placeholder="ghp_…"
        title="Personal Access Token mit Lesezugriff auf die Repositories"
        spellcheck="false"
        autocomplete="off"
        aria-invalid={tokenError ? 'true' : undefined}
        aria-describedby={tokenError ? 'github-token-error' : undefined}
        bind:value={token}
        oninput={() => (tokenError = '')}
        onkeydown={(event) => {
          if (event.key !== 'Enter') return;
          event.preventDefault();
          void connectToken();
        }}
      />
      <button
        type="button"
        class="secondary-button compact"
        disabled={!token.trim()}
        aria-disabled={busy ? 'true' : undefined}
        onclick={() => void connectToken()}
        >{phase === 'connecting' ? 'Wird verbunden …' : 'Verbinden'}</button
      >
    </div>
    {#if tokenError}<p class="field-error" id="github-token-error" role="alert">
        {tokenError}
      </p>{/if}
  </div>
</div>
