<script lang="ts">
  import { untrack } from 'svelte';
  import Icon from './Icon.svelte';
  import {
    BUILT_IN_SHARE,
    agentShare,
    parseShareNames,
    shareText,
    type ShareKind,
  } from './profileShare';
  import type { SettingsDraft } from './settingsDraft.svelte';
  import type { AgentId } from './types';

  // The extra names the profiles of one agent share (#35), staged like
  // every setting; the native side checks them again on „Speichern“. Below
  // them, collapsed, the names the sync handles itself (#37).
  let { agent, draft }: { agent: AgentId; draft: SettingsDraft } = $props();

  const FIELDS = [
    {
      kind: 'dirs',
      label: 'Weitere Ordner',
      noun: 'Ordner',
      example: '.caveman-sessions',
    },
    {
      kind: 'files',
      label: 'Weitere Dateien',
      noun: 'Dateien',
      example: 'orca-state.json',
    },
  ] as const;
  const same = (a: readonly string[], b: readonly string[]) =>
    a.length === b.length && a.every((name, index) => name === b[index]);

  const share = $derived(agentShare(draft.value, agent));
  /** What was typed; it follows the staged list when that changes elsewhere. */
  let text = $state(
    untrack(() => ({
      dirs: shareText(share.dirs),
      files: shareText(share.files),
    })),
  );
  $effect(() => {
    for (const { kind } of FIELDS) {
      const names = share[kind];
      if (!same(parseShareNames(untrack(() => text[kind])), names))
        text[kind] = shareText(names);
    }
  });

  function edit(kind: ShareKind, value: string) {
    text[kind] = value;
    draft.setShare(agent, kind, parseShareNames(value));
  }
</script>

<div class="field-grid profile-share">
  {#each FIELDS as field (field.kind)}
    {@const id = `share-${field.kind}`}
    {@const problem = draft.problem(`agents.${agent}.share.${field.kind}`)}
    <label for={id}>{field.label}</label>
    <div class="field-cell">
      <input
        {id}
        class="text-field mono"
        type="text"
        placeholder={field.example}
        spellcheck="false"
        autocomplete="off"
        title="Durch Komma oder Leerzeichen getrennt"
        aria-invalid={problem ? 'true' : undefined}
        aria-describedby={problem ? `${id}-error` : undefined}
        value={text[field.kind]}
        oninput={(event) => edit(field.kind, event.currentTarget.value)}
      />
      {#if problem}<p id={`${id}-error`} class="field-error">
          {problem}
        </p>{/if}
    </div>
  {/each}
  <details class="profile-builtin">
    <summary>
      <Icon name="down" size={12} />Bereits abgeglichen
    </summary>
    <dl>
      {#each FIELDS as field (field.kind)}
        <dt>{field.noun}</dt>
        <dd class="mono">
          {#each BUILT_IN_SHARE[field.kind] as name, index (name)}{index
              ? ', '
              : ''}<span>{name}</span>{/each}
        </dd>
      {/each}
    </dl>
  </details>
</div>
