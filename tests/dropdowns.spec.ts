import { choose } from './support/dropdown';
import { expect, test, type Page } from './support/test';

/*
 * The composer's dropdowns (#5): one styled list for model, „Denken“ and
 * „Session“, the model list unfiltered until something is typed, and the
 * folder field's GitHub menu — in the labelled demo.
 */
const composer = (page: Page) => page.locator('form.composer');
const model = (page: Page) =>
  composer(page).getByRole('combobox', { name: 'Modell' });
const models = (page: Page) =>
  page.getByRole('listbox', { name: 'Modelle' }).getByRole('option');
const modelIds = (page: Page) => models(page).locator('.choice-label');

test('the model list opens unfiltered; only typing filters it', async ({
  page,
}) => {
  await page.goto('/');
  const field = model(page);
  await field.fill('opus');
  await field.press('Escape');
  await expect(models(page)).toHaveCount(0);
  // Reopened on a chosen model: every model, the chosen one checked and active.
  await field.click();
  await expect(models(page)).toHaveCount(5);
  const opus = models(page).filter({
    has: page.locator('.choice-label', { hasText: /^opus$/ }),
  });
  await expect(opus).toHaveAttribute('data-current', 'true');
  await expect(opus).toHaveAttribute('aria-selected', 'true');
  await field.press('Escape');
  await field.fill('opus');
  await expect(models(page)).toHaveCount(2);
  await field.fill('son');
  await expect(modelIds(page)).toHaveText(['sonnet']);
  await field.press('Enter');
  await expect(field).toHaveValue('sonnet');
  await expect(models(page)).toHaveCount(0);
});

test('the caret toggles the full model list and keeps the field focused', async ({
  page,
}) => {
  await page.goto('/');
  const toggle = composer(page).locator('.chip-field .dropdown-toggle');
  await toggle.click();
  await expect(models(page)).toHaveCount(5);
  await expect(model(page)).toBeFocused();
  await toggle.click();
  await expect(models(page)).toHaveCount(0);
  await expect(model(page)).toBeFocused();
});

test('a dropdown chip: keys pick, Escape closes only the list, Enter starts', async ({
  page,
}) => {
  await page.goto('/');
  const effort = composer(page).getByRole('combobox', { name: 'Denken' });
  await effort.focus();
  await effort.press('ArrowDown');
  const list = page.getByRole('listbox', { name: 'Denken' });
  await expect(list).toBeVisible();
  await expect(effort).toHaveAttribute('aria-expanded', 'true');
  await effort.press('ArrowDown');
  await effort.press('Enter');
  await expect(list).toHaveCount(0);
  await expect(effort).toHaveAttribute('data-value', 'low');
  await expect(effort).toHaveText('Niedrig');
  await effort.press('ArrowDown');
  await effort.press('Escape');
  await expect(list).toHaveCount(0);
  await expect(effort).toBeFocused();
  // Type-ahead on the closed chip, then a click picks.
  await effort.press('h');
  await effort.press('Enter');
  await expect(effort).toHaveAttribute('data-value', 'high');
  await choose(effort, 'Standard');
  await expect(effort).toHaveAttribute('data-value', 'default');
  // Enter on the closed chip starts like anywhere in the composer.
  await composer(page)
    .getByRole('combobox', { name: 'Ordner oder Repo' })
    .fill('C:\\Users\\demo\\Projekte\\traygent');
  await effort.focus();
  await effort.press('Enter');
  await expect(
    page
      .getByRole('status')
      .filter({ hasText: 'Claude Code gestartet (Demo).' }),
  ).toBeVisible();
});

test('the GitHub button lists recent repositories; a pick fills the field', async ({
  page,
}) => {
  await page.goto('/');
  await composer(page)
    .getByRole('button', { name: 'GitHub-Repositories' })
    .click();
  const menu = page.getByRole('menu', { name: 'GitHub-Repositories' });
  const items = menu.getByRole('menuitem');
  await expect(items).toHaveCount(8);
  await expect(items.first()).toBeFocused();
  await items.first().press('Enter');
  await expect(menu).toHaveCount(0);
  await expect(
    composer(page).getByRole('combobox', { name: 'Ordner oder Repo' }),
  ).toHaveValue('demo-user/traygent');
});

test('without an account the GitHub button leads to Settings → GitHub', async ({
  page,
}) => {
  await page.goto('/');
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await dialog.getByRole('tab', { name: 'GitHub' }).click();
  await dialog.getByRole('button', { name: 'Trennen' }).click();
  await dialog
    .getByRole('group', { name: 'demo-user trennen?' })
    .getByRole('button', { name: 'Trennen' })
    .click();
  await expect(
    dialog.getByRole('button', { name: 'Lokale Anmeldung prüfen' }),
  ).toBeFocused();
  await dialog.getByRole('button', { name: 'Abbrechen', exact: true }).click();
  await composer(page)
    .getByRole('button', { name: 'GitHub-Repositories' })
    .click();
  await page.getByRole('menuitem', { name: 'GitHub verbinden …' }).click();
  await expect(dialog.getByRole('tab', { name: 'GitHub' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
});
