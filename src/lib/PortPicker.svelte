<script lang="ts">
  import Icon from './Icon.svelte';
  import {
    EXPOSED,
    addPort,
    exposedIp,
    portListProblem,
    presetPort,
  } from './ports';
  import type { PortPreset } from './types';

  // Published ports of a Docker session: the presets as checkboxes, custom
  // entries as removable chips, one field to add one. Used by the composer's
  // „Ports“ chip and by Einstellungen → Umgebung → Docker.
  let {
    id,
    label,
    presets,
    value,
    onChange,
  }: {
    /** Prefix of the element ids; unique per picker on the page. */
    id: string;
    /** Accessible name of the group, e.g. „Ports“. */
    label: string;
    presets: readonly PortPreset[];
    /** Normalized entries, e.g. `127.0.0.1:3000:3000`. */
    value: readonly string[];
    onChange: (next: string[]) => void;
  } = $props();

  let text = $state('');
  /** Why the typed entry cannot be added; cleared on every keystroke. */
  let typed = $state('');
  let input = $state<HTMLInputElement>();
  let list = $state<HTMLUListElement>();
  const presetEntries = $derived(
    new Set(presets.map((p) => presetPort(p.port))),
  );
  const custom = $derived(value.filter((entry) => !presetEntries.has(entry)));
  const problem = $derived(typed || portListProblem(value));
  const errorId = $derived(`${id}-error`);

  function toggle(entry: string, on: boolean) {
    const rest = value.filter((item) => item !== entry);
    onChange(on ? [...rest, entry] : rest);
  }
  function add() {
    if (!text.trim()) return;
    const result = addPort(value, text);
    if (!result.list) {
      typed = result.problem;
      return;
    }
    onChange(result.list);
    text = '';
    typed = '';
  }
  /** Focus moves to the next chip, else the field: the removed one is gone. */
  function remove(entry: string, index: number) {
    const buttons = list?.querySelectorAll<HTMLElement>('.port-remove');
    const next = buttons?.[index + 1] ?? buttons?.[index - 1] ?? input;
    next?.focus();
    toggle(entry, false);
  }
</script>

<div
  class="port-picker"
  role="group"
  aria-label={label}
  title="Dienst im Container auf 0.0.0.0 binden"
>
  <div class="port-presets">
    {#each presets as preset (preset.port)}
      {@const entry = presetPort(preset.port)}
      <label class="port-preset" title={`${entry} · ${preset.label}`}
        ><input
          type="checkbox"
          aria-label={`${preset.port} ${preset.label}`}
          checked={value.includes(entry)}
          onchange={(event) => toggle(entry, event.currentTarget.checked)}
        /><span class="port-number">{preset.port}</span><span class="port-name"
          >{preset.label}</span
        ></label
      >
    {/each}
  </div>
  {#if custom.length}
    <ul class="port-custom" aria-label={`Eigene ${label}`} bind:this={list}>
      {#each custom as entry, index (entry)}
        {@const exposed = exposedIp(entry) !== null}
        <li
          class="chip chip-static port-chip"
          data-warn={exposed ? 'true' : undefined}
          title={exposed ? `${entry} · ${EXPOSED}` : undefined}
        >
          <span class="mono">{entry}</span>{#if exposed}<span class="sr-only"
              >, {EXPOSED}</span
            >{/if}<button
            type="button"
            class="port-remove"
            aria-label={`${entry} entfernen`}
            title="Entfernen"
            onclick={() => remove(entry, index)}
            ><Icon name="close" size={11} /></button
          >
        </li>
      {/each}
    </ul>
  {/if}
  <div class="port-add">
    <input
      bind:this={input}
      bind:value={text}
      class="text-field mono"
      type="text"
      aria-label="Eigener Port"
      placeholder="Eigener Port"
      title="PORT, HOST:CONTAINER oder IP:HOST:CONTAINER, Bereich A-B, optional /udp"
      spellcheck="false"
      autocomplete="off"
      aria-invalid={typed ? 'true' : undefined}
      aria-describedby={problem ? errorId : undefined}
      oninput={() => (typed = '')}
      onkeydown={(event) => {
        if (event.key !== 'Enter') return;
        // Enter adds here; it never starts the composer's session.
        event.preventDefault();
        event.stopPropagation();
        add();
      }}
    /><button
      type="button"
      class="icon-button small"
      aria-label="Port hinzufügen"
      title="Hinzufügen (Enter)"
      onclick={add}><Icon name="plus" size={14} /></button
    >
  </div>
  {#if problem}<p id={errorId} class="field-error">
      {problem}
    </p>{/if}
</div>
