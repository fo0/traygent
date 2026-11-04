<script lang="ts">
  import Icon from './Icon.svelte';
  import type { DownloadProgress, UpdateInfo } from './types';
  import { changelogSummary, releaseDate, type UpdatePhase } from './updates';

  let {
    currentVersion,
    native,
    info,
    phase,
    progress,
    error,
    checkedAt,
    onCheck,
    onInstall,
  }: {
    currentVersion: string;
    native: boolean;
    info: UpdateInfo | null;
    phase: UpdatePhase;
    progress: DownloadProgress;
    error: string;
    checkedAt: string;
    onCheck: () => void;
    /** Downloads, verifies and installs in one go; the app then restarts. */
    onInstall: () => void;
  } = $props();
  const busy = $derived(
    ['checking', 'downloading', 'verifying', 'installing'].includes(phase),
  );
  const percent = $derived(
    progress.total
      ? Math.min(100, Math.floor((progress.downloaded / progress.total) * 100))
      : undefined,
  );
  const published = $derived(info ? releaseDate(info.date) : '');
  const bullets = $derived(info ? changelogSummary(info.notes) : []);
</script>

<section class="update-panel" aria-label="App-Update">
  <div class="update-row">
    <div class="update-version">
      <strong>Version {currentVersion}</strong>
      {#if native && checkedAt}<span>Zuletzt geprüft {checkedAt}</span>{/if}
    </div>
    {#if native}<button
        class="secondary-button"
        disabled={busy}
        onclick={onCheck}
        ><Icon name="refresh" size={14} />Nach Updates suchen</button
      >{/if}
  </div>
  {#if !native}
    <p class="update-line">In der Demo nicht verfügbar.</p>
  {:else if phase === 'current'}
    <p class="update-line update-ok" role="status">
      <Icon name="check" size={14} />Aktuell.
    </p>
  {:else if info}
    <div class="update-release">
      <p class="update-title">
        <strong>Version {info.version} verfügbar</strong>{#if published}<span
            >{published}</span
          >{/if}
      </p>
      {#if bullets.length}<ul class="update-notes" aria-label="Änderungen">
          {#each bullets as bullet, index (index)}<li>{bullet}</li>{/each}
        </ul>{/if}
      {#if phase === 'downloading'}
        <div class="update-progress" role="status">
          <label for="update-download"
            >Lädt …{percent === undefined ? '' : ` ${percent} %`}</label
          >
          {#if percent === undefined}<progress id="update-download"
            ></progress>{:else}<progress
              id="update-download"
              max="100"
              value={percent}
            ></progress>{/if}
        </div>
      {:else if phase === 'verifying'}
        <p class="update-line" role="status">Wird geprüft …</p>
      {:else if phase === 'installing'}
        <p class="update-line" role="status">
          Wird installiert – Traygent startet neu …
        </p>
      {:else}
        {#if error}<div class="alert danger" role="alert">
            <Icon name="warning" size={14} /><span>{error}</span>
          </div>{/if}
        <button class="primary-button" disabled={busy} onclick={onInstall}
          >{error ? 'Erneut versuchen' : 'Installieren'}</button
        >
      {/if}
    </div>
  {/if}
  {#if error && !info}<div class="alert danger" role="alert">
      <Icon name="warning" size={14} /><span>{error}</span>
    </div>{/if}
</section>
