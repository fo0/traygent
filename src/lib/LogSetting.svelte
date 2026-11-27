<script lang="ts">
  import { DEBUG_TOOLTIP, LOG_LEVELS } from './logLevel';
  import { errorMessage } from './format';
  import type { SettingsDraft } from './settingsDraft.svelte';

  // „Protokoll“: the level is staged like every setting; „Ordner
  // öffnen“ acts at once and exists only natively — the browser demo writes
  // no log file.
  let {
    draft,
    native,
    onOpenFolder,
  }: {
    draft: SettingsDraft;
    native: boolean;
    onOpenFolder: () => Promise<void>;
  } = $props();
  let error = $state('');

  async function open() {
    error = '';
    try {
      await onOpenFolder();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }
</script>

<section class="log-setting" aria-labelledby="log-heading">
  <h3 id="log-heading">Protokoll</h3>
  <div class="log-row">
    <div class="theme-options" role="radiogroup" aria-labelledby="log-heading">
      {#each LOG_LEVELS as level (level.value)}
        <label
          class="theme-option"
          title={level.value === 'debug' ? DEBUG_TOOLTIP : undefined}
        >
          <input
            type="radio"
            name="log-level"
            value={level.value}
            bind:group={draft.value.logLevel}
          />
          <span>{level.label}</span>
        </label>
      {/each}
    </div>
    {#if native}<button
        type="button"
        class="secondary-button"
        onclick={() => void open()}>Ordner öffnen</button
      >{/if}
  </div>
  {#if error}<p class="field-error" role="alert">{error}</p>{/if}
</section>
