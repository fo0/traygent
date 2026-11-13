<script lang="ts">
  import Dropdown from './Dropdown.svelte';
  import Icon from './Icon.svelte';
  import ProfileMark from './ProfileMark.svelte';
  import { DEFAULT_PROFILE } from './profiles';
  import type { ProfileStatus } from './types';

  // „Profil“: which sign-in the next start uses. A mark only, so every
  // agent's chips keep one row beside „Starten“ at 480 px: the person icon
  // for „Standard“, else the profile's initial; the label is the accessible
  // name and the tooltip.
  let {
    profiles,
    value,
    onChange,
    onEnter,
  }: {
    /** „Standard“ first, as `Snapshot.profiles` lists them. */
    profiles: readonly ProfileStatus[];
    value: string;
    onChange: (id: string) => void;
    onEnter: () => void;
  } = $props();

  const OUT = 'Nicht angemeldet';
  const chosen = $derived(profiles.find((profile) => profile.id === value));
  const label = $derived(chosen?.label ?? value);
  const out = $derived(!!chosen && !chosen.signedIn);
  const items = $derived(
    profiles.map((profile) => ({
      id: profile.id,
      label: profile.label,
      meta: profile.signedIn ? undefined : OUT,
      title: profile.signedIn ? profile.account : OUT,
    })),
  );
  const title = $derived(
    `Profil: ${label}${out ? ` · ${OUT}` : chosen?.account ? ` · ${chosen.account}` : ''}`,
  );
</script>

<span class="profile-chip" data-warn={out ? 'true' : undefined}>
  <Dropdown
    id="composer-profile"
    label="Profil"
    glyph={`Profil: ${label}${out ? ` (${OUT})` : ''}`}
    {items}
    {value}
    {onChange}
    {title}
    {onEnter}
    >{#snippet lead()}{#if value === DEFAULT_PROFILE}<Icon
          name="user"
          size={14}
        />{:else}<ProfileMark {label} />{/if}{/snippet}</Dropdown
  >
</span>
