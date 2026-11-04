<script lang="ts">
  import OptionFields from './OptionFields.svelte';
  import type { SettingsDraft } from './settingsDraft.svelte';
  import type { AgentCatalogEntry } from './types';

  // The staged defaults of one agent: every new start begins from these.
  let { entry, draft }: { entry: AgentCatalogEntry; draft: SettingsDraft } =
    $props();
</script>

<section class="settings-group" aria-labelledby="defaults-heading">
  <h3 id="defaults-heading">Standards</h3>
  <OptionFields
    {entry}
    options={draft.value.agents[entry.id]}
    onChange={(patch) => Object.assign(draft.value.agents[entry.id], patch)}
    prefix="default"
    modelProblem={draft.problem(`agents.${entry.id}.model`)}
    argsProblem={draft.problem(`agents.${entry.id}.extraArgs`)}
  />
</section>
