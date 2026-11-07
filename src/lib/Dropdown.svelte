<script lang="ts">
  import type { Snippet } from 'svelte';
  import ChoiceList, { type ChoiceItem } from './ChoiceList.svelte';
  import Icon from './Icon.svelte';
  import { menuIndex, pressClosedPopup } from './menu';

  // A select-only combobox (WAI-ARIA APG) in place of the native `select`,
  // whose popup the WebView draws in its own style: the same list as the
  // model field everywhere. Focus stays on the button; the list follows
  // `aria-activedescendant`.
  let {
    id,
    label,
    items,
    value,
    onChange,
    title,
    disabled = false,
    variant = 'chip',
    onEnter,
    lead,
  }: {
    /** Element id; the list's id derives from it. */
    id: string;
    /** Accessible name of the control and its list. */
    label: string;
    items: readonly ChoiceItem[];
    value: string;
    onChange: (value: string) => void;
    title?: string | undefined;
    disabled?: boolean;
    /** A composer chip or a full-width form field. */
    variant?: 'chip' | 'field';
    /** Enter on the closed control, e.g. the composer's start. */
    onEnter?: (() => void) | undefined;
    /** The chip's leading mark. */
    lead?: Snippet;
  } = $props();

  const listId = $derived(`${id}-list`);
  let open = $state(false);
  let active = $state(0);
  const selected = $derived(items.findIndex((item) => item.id === value));
  /** An unknown stored value is shown as it is. */
  const text = $derived(items[selected]?.label ?? value);

  let typed = '';
  let typedAt = 0;

  function show(index = selected) {
    active = Math.max(0, Math.min(index, items.length - 1));
    open = items.length > 0;
  }
  function pick(index: number) {
    const item = items[index];
    open = false;
    if (item && item.id !== value) onChange(item.id);
  }
  /** Type-ahead: the next option whose label starts with the typed text. */
  function seek(key: string, from: number) {
    const now = Date.now();
    typed = now - typedAt < 700 ? typed + key.toLowerCase() : key.toLowerCase();
    typedAt = now;
    const start = typed.length > 1 ? from : from + 1;
    for (let step = 0; step < items.length; step++) {
      const index = (start + step) % items.length;
      if (items[index]?.label.toLowerCase().startsWith(typed)) return index;
    }
    return null;
  }
  function keydown(event: KeyboardEvent) {
    const { key } = event;
    if (!open) {
      if (key === 'Enter' && onEnter) {
        event.preventDefault();
        onEnter();
      } else if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(key)) {
        event.preventDefault();
        show();
      } else if (key === 'Home' || key === 'End') {
        event.preventDefault();
        show(key === 'Home' ? 0 : items.length - 1);
      } else if (key.length === 1 && !event.ctrlKey && !event.metaKey) {
        const found = seek(key, selected);
        if (found !== null) show(found);
      }
      return;
    }
    if (key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      open = false;
    } else if (key === 'Enter' || key === ' ') {
      event.preventDefault();
      pick(active);
    } else if (key === 'Tab') open = false;
    else if (key === 'ArrowUp' && event.altKey) {
      event.preventDefault();
      pick(active);
    } else {
      const next =
        menuIndex(key, active, items.length) ??
        (key.length === 1 && !event.ctrlKey && !event.metaKey
          ? seek(key, active)
          : null);
      if (next === null) return;
      event.preventDefault();
      active = next;
    }
  }
</script>

<div class={['dropdown', variant === 'field' && 'field']}>
  <button
    {id}
    type="button"
    class={variant === 'chip'
      ? 'chip chip-button dropdown-chip'
      : 'text-field dropdown-field'}
    role="combobox"
    aria-label={variant === 'chip' ? label : undefined}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={open ? listId : undefined}
    aria-activedescendant={open ? `${listId}-${active}` : undefined}
    data-value={value}
    {title}
    {disabled}
    onclick={() => (open ? (open = false) : show())}
    onkeydown={keydown}
    onblur={() => {
      // A press elsewhere that closes the list does nothing else.
      if (open) pressClosedPopup();
      open = false;
    }}
    >{@render lead?.()}<span class="dropdown-text">{text}</span><Icon
      name="down"
      size={10}
      class="dropdown-caret"
    /></button
  >
  {#if open}
    <ChoiceList
      id={listId}
      {label}
      {items}
      current={value}
      {active}
      onPick={pick}
      onHover={(index) => (active = index)}
    />
  {/if}
</div>
