<script lang="ts">
  import Icon, { type IconName } from './Icon.svelte';
  import { applyTheme, type ThemePreference } from './theme';

  const OPTIONS: { value: ThemePreference; label: string; icon: IconName }[] = [
    { value: 'system', label: 'System', icon: 'monitor' },
    { value: 'light', label: 'Hell', icon: 'sun' },
    { value: 'dark', label: 'Dunkel', icon: 'moon' },
  ];

  // A choice previews at once; the dialog's „Speichern“ keeps it and
  // „Abbrechen“ restores the saved one.
  let { value = $bindable() }: { value: ThemePreference } = $props();

  function choose(next: ThemePreference) {
    value = next;
    applyTheme(next);
  }
</script>

<section class="theme-setting" aria-labelledby="theme-heading">
  <h3 id="theme-heading">Darstellung</h3>
  <div class="theme-options" role="radiogroup" aria-labelledby="theme-heading">
    {#each OPTIONS as option (option.value)}
      <label class="theme-option">
        <input
          type="radio"
          name="theme-preference"
          value={option.value}
          checked={value === option.value}
          onchange={() => choose(option.value)}
        />
        <Icon name={option.icon} size={15} /><span>{option.label}</span>
      </label>
    {/each}
  </div>
</section>
