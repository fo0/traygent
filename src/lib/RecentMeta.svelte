<script lang="ts">
  import Icon from './Icon.svelte';
  import ModelBadge from './ModelBadge.svelte';
  import ProfileMark from './ProfileMark.svelte';
  import { PERMISSION_ICONS, type EnvInfo } from './agents';
  import { relativeTime } from './format';
  import { effortLevel } from './optionMarks';
  import type { PermissionChoice } from './types';

  /*
   * A recent row's fixed-size parts in fixed columns (owner request, #20):
   * ports · repo · environment · profile · model · thinking · permission ·
   * time; the profile column only while some row has a profile. An
   * absent part keeps its column as an empty, hidden slot, so every icon
   * sits in the same column from row to row. The ports chip is the only
   * part of variable width: it goes first, so it never moves the others.
   */
  let {
    repo,
    env,
    profile,
    profileColumn,
    model,
    modelLabel,
    thinks,
    effort,
    effortLabel,
    portsText,
    portsTitle,
    exposed,
    permission,
    lastStartedAt,
    now,
  }: {
    /** The repository's accessible name, or `''`. */
    repo: string;
    env: EnvInfo;
    /** The profile's label, `''` for „Standard“. */
    profile: string;
    profileColumn: boolean;
    model: string;
    modelLabel: string;
    /** Whether the agent sends a thinking level with this model. */
    thinks: boolean;
    effort: string;
    effortLabel: string;
    /** The chip's text, or `''` without ports. */
    portsText: string;
    portsTitle: string;
    /** A port the network reaches. */
    exposed: boolean;
    permission: PermissionChoice;
    lastStartedAt: string;
    now: number;
  } = $props();
</script>

<span class="recent-meta" data-profiles={profileColumn ? 'true' : undefined}>
  {#if portsText}<span
      class="chip chip-static recent-ports"
      data-warn={exposed ? 'true' : undefined}
      role="img"
      aria-label={portsTitle}
      ><Icon name="ports" size={11} /><span>{portsText}</span></span
    >{:else}<span class="recent-slot" aria-hidden="true"></span>{/if}
  {#if repo}<span class="recent-repo" role="img" aria-label={repo}
      ><Icon name="github" size={13} /></span
    >{:else}<span class="recent-slot" aria-hidden="true"></span>{/if}
  <span class="recent-env" role="img" aria-label={env.label}
    ><Icon name={env.icon} size={13} /></span
  >
  {#if profile}<ProfileMark
      label={profile}
      name={`Profil ${profile}`}
    />{:else if profileColumn}<span
      class="recent-slot recent-profile-slot"
      aria-hidden="true"
    ></span>{/if}
  {#if model !== 'default'}<ModelBadge {model} label={modelLabel} />{:else}<span
      class="recent-slot"
      aria-hidden="true"
    ></span>{/if}
  {#if thinks}<span
      class="recent-effort"
      data-level={effortLevel(effort)}
      role="img"
      aria-label={`Denken: ${effortLabel}`}
      ><Icon name="signal" size={14} level={effortLevel(effort)} /></span
    >{:else}<span class="recent-slot" aria-hidden="true"></span>{/if}
  <span
    class="recent-permission"
    data-warn={permission.danger ? 'true' : undefined}
    role="img"
    aria-label={`Freigabe: ${permission.label}`}
    ><Icon name={PERMISSION_ICONS[permission.id]} size={13} /></span
  >
  <time class="recent-time" datetime={lastStartedAt}
    >{relativeTime(lastStartedAt, now)}</time
  >
</span>
