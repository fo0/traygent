import type { Locator } from '@playwright/test';

/**
 * Picks the option named `label` in a dropdown (`Dropdown.svelte`, a
 * select-only combobox): opens it with a click and clicks the option.
 */
export async function choose(dropdown: Locator, label: string) {
  await dropdown.click();
  await dropdown
    .page()
    .getByRole('listbox')
    .getByRole('option', { name: label, exact: true })
    .click();
}
