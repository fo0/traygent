<script lang="ts">
  import ChoiceList, { type ChoiceItem } from './ChoiceList.svelte';
  import Icon from './Icon.svelte';
  import { pressClosedPopup } from './menu';
  import { matchChoices } from './choices';
  import type { Choice } from './types';

  // The free model field (any model string, catalog models suggested): an
  // editable combobox in place of the native `datalist`, which filters by
  // the current text and so offered only the chosen model. Opening (click,
  // the caret, ↓) lists every model; only typing filters.
  let {
    id,
    models,
    value,
    onChange,
    title,
    invalid = '',
    describedBy,
    variant = 'chip',
    onEnter,
  }: {
    id: string;
    /** The catalog models; `default` stands for the empty field. */
    models: readonly Choice[];
    /** The model id; `default` shows an empty field. */
    value: string;
    onChange: (value: string) => void;
    title: string;
    /** Why the typed model is refused; marks the field. */
    invalid?: string;
    describedBy?: string | undefined;
    variant?: 'chip' | 'field';
    /** Enter on the closed field, e.g. the composer's start. */
    onEnter?: (() => void) | undefined;
  } = $props();

  const listId = $derived(`${id}-list`);
  const text = $derived(value === 'default' ? '' : value);
  const all = $derived<ChoiceItem[]>(
    models
      .filter((model) => model.id !== 'default')
      .map((model) => ({
        id: model.id,
        label: model.id,
        meta: model.label === model.id ? undefined : model.label,
        title: model.hint || undefined,
      })),
  );
  let open = $state(false);
  /** Text typed since the list opened; null lists every model. */
  let query = $state<string | null>(null);
  let active = $state(0);
  const items = $derived(query === null ? all : matchChoices(all, query));
  const expanded = $derived(open && items.length > 0);
  const current = $derived(Math.min(active, items.length - 1));
  let input = $state<HTMLInputElement>();

  function show() {
    query = null;
    active = Math.max(
      0,
      all.findIndex((item) => item.id === value),
    );
    open = true;
  }
  function pick(index: number) {
    const item = items[index];
    open = false;
    if (item) onChange(item.id);
  }
  function keydown(event: KeyboardEvent) {
    const step =
      event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0;
    if (step) {
      event.preventDefault();
      if (!expanded) show();
      else if (event.altKey && step < 0) open = false;
      else active = (current + step + items.length) % items.length;
    } else if (event.key === 'Enter') {
      event.preventDefault();
      if (expanded) pick(current);
      else onEnter?.();
    } else if (event.key === 'Escape' && expanded) {
      event.preventDefault();
      event.stopPropagation();
      open = false;
    }
  }
  /** The caret toggles the full list and keeps the field's focus. */
  function toggle(event: MouseEvent) {
    event.preventDefault();
    if (expanded) open = false;
    else show();
    input?.focus();
  }
</script>

<div class={['dropdown', variant === 'field' && 'field']}>
  <!-- The field's box is no label: its text would join the accessible name. -->
  <svelte:element
    this={variant === 'chip' ? 'label' : 'div'}
    class={variant === 'chip' ? 'chip chip-field' : 'dropdown-field-box'}
    data-invalid={invalid ? 'true' : undefined}
    title={variant === 'chip' ? title : undefined}
  >
    {#if variant === 'chip'}<Icon name="cpu" size={13} />{/if}<input
      bind:this={input}
      {id}
      class={variant === 'chip' ? 'chip-input' : 'text-field'}
      type="text"
      role="combobox"
      aria-label={variant === 'chip' ? 'Modell' : undefined}
      aria-autocomplete="list"
      aria-expanded={expanded}
      aria-controls={expanded ? listId : undefined}
      aria-activedescendant={expanded ? `${listId}-${current}` : undefined}
      aria-invalid={invalid ? 'true' : undefined}
      aria-describedby={describedBy}
      placeholder={variant === 'chip' ? 'Modell' : 'Standard'}
      title={variant === 'field' ? title : undefined}
      spellcheck="false"
      autocomplete="off"
      value={text}
      oninput={(event) => {
        const typed = event.currentTarget.value;
        onChange(typed.trim() || 'default');
        query = typed.trim();
        active = 0;
        open = true;
      }}
      onclick={() => {
        if (!expanded) show();
      }}
      onkeydown={keydown}
      onblur={() => {
        if (expanded) pressClosedPopup();
        open = false;
      }}
    /><!-- Pointer only; keys open the list on the field itself. The click
    is cancelled: inside the label it would reach the field and reopen. -->
    <span
      class="dropdown-toggle"
      aria-hidden="true"
      onmousedown={toggle}
      onclick={(event) => event.preventDefault()}
      ><Icon name="down" size={10} class="dropdown-caret" /></span
    >
  </svelte:element>
  {#if expanded}
    <ChoiceList
      id={listId}
      label="Modelle"
      {items}
      current={value}
      active={current}
      onPick={pick}
      onHover={(index) => (active = index)}
    />
  {/if}
</div>
