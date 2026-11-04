<script lang="ts">
  import Icon from './Icon.svelte';
  import { AGENT_ICONS } from './agents';
  import type { AgentId } from './types';

  // The agent's product mark; its name is the accessible name. No tooltip:
  // the row's start covers the badge and names the agent in its own.
  let { agent, label }: { agent: AgentId; label: string } = $props();
</script>

<span class="agent-badge" role="img" aria-label={label}
  ><Icon name={AGENT_ICONS[agent]} size={13} /></span
>
