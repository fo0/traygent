<script lang="ts">
  import Icon from './Icon.svelte';
  import ProfileMark from './ProfileMark.svelte';
  import { AGENT_ICONS } from './agents';
  import type { AgentId } from './types';

  // The agent's product mark; its name is the accessible name. No tooltip:
  // the row's start covers the badge and names the agent (and the profile)
  // in its own. A profile other than „Standard“ sits on the tile's corner as
  // its initial (#33), so it takes no width from the folder name.
  let {
    agent,
    label,
    profile = '',
  }: {
    agent: AgentId;
    label: string;
    /** The profile's label, `''` for „Standard“. */
    profile?: string;
  } = $props();
</script>

<span class="agent-tile"
  ><span class="agent-badge" role="img" aria-label={label}
    ><Icon name={AGENT_ICONS[agent]} size={13} /></span
  >{#if profile}<ProfileMark
      label={profile}
      name={`Profil ${profile}`}
    />{/if}</span
>
