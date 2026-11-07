/**
 * Choices of the dropdowns (`Dropdown`, `ModelCombo`): catalog choices as
 * list items and the model field's filter.
 */

import type { ChoiceItem } from './ChoiceList.svelte';
import type { Choice } from './types';

/** An item a dropdown lists; `label` and `meta` are what the user reads. */
export interface Matchable {
  label: string;
  meta?: string | undefined;
}

/**
 * The items whose label or meta contains every word of `query`, ignoring
 * case, in their catalog order; an empty query keeps them all.
 */
export function matchChoices<T extends Matchable>(
  items: readonly T[],
  query: string,
): T[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return [...items];
  return items.filter((item) => {
    const text = `${item.label} ${item.meta ?? ''}`.toLowerCase();
    return words.every((word) => text.includes(word));
  });
}

/**
 * Catalog choices as dropdown items; the hint becomes the tooltip. A stored
 * value the catalog no longer lists stays selectable as it is.
 */
export function toItems(
  choices: readonly Choice[],
  value?: string,
): ChoiceItem[] {
  const items = choices.map((choice) => ({
    id: choice.id,
    label: choice.label,
    title: choice.hint || undefined,
  }));
  if (value !== undefined && !choices.some((choice) => choice.id === value))
    items.unshift({ id: value, label: value, title: undefined });
  return items;
}
