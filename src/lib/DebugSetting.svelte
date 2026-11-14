<script lang="ts">
  import type { SettingsDraft } from './settingsDraft.svelte';

  // „Debug“ (#41): staged like every setting; with it a profile sign-in
  // opens the visible terminal instead of running hidden.
  let { draft }: { draft: SettingsDraft } = $props();
</script>

<section class="debug-setting" aria-labelledby="debug-heading">
  <h3 id="debug-heading">Fehlersuche</h3>
  <label
    class="switch-row"
    title="Anmeldung im sichtbaren Terminal, Fehler mit Ausgabe"
  >
    <input
      type="checkbox"
      class="switch-input"
      bind:checked={draft.value.debug}
    />
    <span class="switch" aria-hidden="true"></span>
    <span>Debug</span>
  </label>
</section>
