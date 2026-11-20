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
    <!-- A read or write is off via aria-disabled: switch and buttons keep
         focus meanwhile (a read also runs whenever the window gets focus). -->
    <label class="startup-option">
      <input
        type="checkbox"
        checked={autostart.checked}
        disabled={!autostart.editable}
        aria-disabled={autostart.busy ? 'true' : undefined}
        aria-describedby={note ? 'startup-status' : undefined}
        onclick={(event) => autostart.busy && event.preventDefault()}
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
          type="button"
          class="secondary-button"
          aria-disabled={autostart.busy ? 'true' : undefined}
          onclick={() => void autostart.read()}
          ><Icon name="refresh" size={14} />Status prüfen</button
        >
        {#if vetoed}
          <button
            type="button"
            class="secondary-button"
            aria-disabled={autostart.busy ? 'true' : undefined}
            onclick={() =>
              !autostart.busy && void autostart.openWindowsSettings()}
            >Windows-Autostart öffnen</button
          >
          {#if autostart.current?.registered}<button
              type="button"
              class="text-button"
              aria-disabled={autostart.busy ? 'true' : undefined}
              onclick={() => !autostart.busy && void autostart.remove()}
              >Autostarteintrag entfernen</button
            >{/if}
        {/if}
      </div>
    {/if}
    {#if autostart.error}<div class="alert danger" role="alert">
        <Icon name="warning" size={16} /><span>{autostart.error}</span>
      </div>{/if}
  {/if}
</section>
