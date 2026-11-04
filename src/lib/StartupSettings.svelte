<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
  import { startupNote, type Autostart } from './autostart.svelte';

  // The switch stages a choice; the dialog's „Speichern“ writes it.
  let { native, autostart }: { native: boolean; autostart: Autostart } =
    $props();
  const note = $derived(startupNote(autostart.current, autostart.busy));
  const status = $derived(autostart.current?.status);
  /** Windows itself blocks or hides Traygent's entry. */
  const vetoed = $derived(
    status === 'disabledByWindows' || status === 'unknownApproval',
  );
  onMount(() => void autostart.read());
</script>

<svelte:window onfocus={() => void autostart.read()} />

<section
  class="startup-panel"
  aria-labelledby="startup-heading"
  aria-busy={autostart.busy}
>
  <h3 id="startup-heading">Autostart</h3>
  {#if !native}
    <p class="field-help">
      Die Browser-Demo ändert keine Windows-Einstellungen.
    </p>
  {:else if status === 'unsupported'}
    <p class="field-help">{note}</p>
  {:else}
    <label class="startup-option">
      <input
        type="checkbox"
        checked={autostart.checked}
        disabled={autostart.busy || !autostart.editable}
        aria-describedby={note ? 'startup-status' : undefined}
        onchange={(event) => {
          autostart.stage(event.currentTarget.checked);
          event.currentTarget.checked = autostart.checked;
        }}
      />
      <span>
        <strong>Mit Windows starten</strong>
        <span>Startet nach der Anmeldung im Tray.</span>
      </span>
    </label>
    {#if note}<p id="startup-status" class="startup-status" role="status">
        {note}
      </p>{/if}
    {#if autostart.loaded && !autostart.editable}
      <div class="startup-actions">
        <button
          class="secondary-button"
          disabled={autostart.busy}
          onclick={autostart.read}
          ><Icon name="refresh" size={14} />Status prüfen</button
        >
        {#if vetoed}
          <button
            class="secondary-button"
            disabled={autostart.busy}
            onclick={autostart.openWindowsSettings}
            >Windows-Autostart öffnen</button
          >
          {#if autostart.current?.registered}<button
              class="text-button"
              disabled={autostart.busy}
              onclick={autostart.remove}>Autostarteintrag entfernen</button
            >{/if}
        {/if}
      </div>
    {/if}
    {#if autostart.error}<div class="alert danger" role="alert">
        <Icon name="warning" size={16} /><span>{autostart.error}</span>
      </div>{/if}
  {/if}
</section>
