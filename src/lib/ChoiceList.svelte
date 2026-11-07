<script module lang="ts">
  export interface ChoiceItem {
    /** The value a pick sets. */
    id: string;
    label: string;
    /** Secondary text on the same line, e.g. the model's display name. */
    meta?: string | undefined;
    /** Tooltip, e.g. the catalog hint. */
    title?: string | undefined;
  }
</script>

<script lang="ts">
  import type { Action } from 'svelte/action';
  import Icon from './Icon.svelte';

  // The one popup list of every dropdown (`Dropdown`, `ModelCombo`): a
  // listbox under its control. Keys stay on the control (combobox pattern);
  // a press keeps the control's focus, a click picks. The chosen value
  // carries a check mark, the active option the hover colour.
  let {
    id,
    label,
    items,
    current,
    active,
    onPick,
    onHover,
  }: {
    id: string;
    /** Accessible name of the list. */
    label: string;
    items: readonly ChoiceItem[];
    /** The chosen value. */
    current: string;
    /** Index of the active option (`aria-activedescendant`). */
    active: number;
    onPick: (index: number) => void;
    onHover: (index: number) => void;
  } = $props();

  const keepFocus = (event: MouseEvent) => event.preventDefault();

  /**
   * Keeps the list on screen (the scrolling content area moves just enough)
   * and the active option inside the list's own scroll box.
   */
  const reveal: Action<HTMLElement, number> = (node, index) => {
    node.scrollIntoView?.({ block: 'nearest' });
    const show = (at: number) => {
      const option = node.children[at];
      if (!(option instanceof HTMLElement)) return;
      const top = option.offsetTop;
      const bottom = top + option.offsetHeight;
      if (top < node.scrollTop) node.scrollTop = top;
      else if (bottom > node.scrollTop + node.clientHeight)
        node.scrollTop = bottom - node.clientHeight;
    };
    show(index);
    return { update: show };
  };
</script>

<ul
  class="menu choice-list"
  role="listbox"
  {id}
  aria-label={label}
  use:reveal={active}
>
  {#each items as item, index (item.id)}
    <li
      id={`${id}-${index}`}
      role="option"
      tabindex="-1"
      class="menu-item choice"
      aria-selected={index === active}
      data-current={item.id === current ? 'true' : undefined}
      title={item.title}
      onmousedown={keepFocus}
      onmousemove={() => index !== active && onHover(index)}
      onclick={() => onPick(index)}
      onkeydown={() => {}}
    >
      <span class="choice-label">{item.label}</span>
      {#if item.meta}<span class="choice-meta">{item.meta}</span>{/if}
      <Icon name="check" size={13} class="choice-check" />
    </li>
  {/each}
</ul>
