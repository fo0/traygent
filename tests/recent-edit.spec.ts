import { expect, test, type Page } from './support/test';

/*
 * „Bearbeiten …“ of a „Zuletzt“ row (#3): the options and ports its ▶ sends,
 * stored without a start. Runs against the labelled browser demo.
 */
const row = (page: Page, name: string) =>
  page.locator('.recent-row').filter({
    has: page.locator('.recent-name', { hasText: new RegExp(`^${name}$`) }),
  });

async function openEdit(page: Page, name: string) {
  await row(page, name)
    .getByRole('button', { name: `Aktionen für ${name}` })
    .click();
  await page.getByRole('menuitem', { name: 'Bearbeiten …' }).click();
  return page.getByRole('dialog', { name });
}

test('edits a local row: Bypass by default, saved without a start', async ({
  page,
}) => {
  await page.goto('/');
  const target = row(page, 'Notizen');
  await expect(target.getByLabel('Freigabe: Nachfragen')).toBeVisible();
  const dialog = await openEdit(page, 'Notizen');
  // A Lokal session publishes no ports.
  await expect(dialog.getByRole('group', { name: 'Ports' })).toHaveCount(0);
  const save = dialog.getByRole('button', { name: 'Speichern' });
  await expect(save).toHaveAttribute('aria-disabled', 'true');
  await dialog.getByRole('radio', { name: 'Bypass' }).check();
  await dialog.getByLabel('Modell').fill('opus');
  await save.click();
  await expect(dialog).toBeHidden();
  await expect(target.getByLabel('Freigabe: Bypass')).toBeVisible();
  await expect(
    page.getByRole('status').filter({ hasText: 'gestartet' }),
  ).toHaveCount(0);
  // Focus returns to the row's „⋯“.
  await expect(
    target.getByRole('button', { name: 'Aktionen für Notizen' }),
  ).toBeFocused();
});

test('an invalid entry blocks „Speichern“; „Abbrechen“ keeps the row', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openEdit(page, 'kunden-portal');
  await expect(dialog.getByRole('group', { name: 'Ports' })).toBeVisible();
  await dialog.getByLabel('Extra-Argumente').fill('"x"');
  await expect(dialog.getByText('Keine Anführungszeichen.')).toBeVisible();
  await expect(
    dialog.getByRole('button', { name: 'Speichern' }),
  ).toHaveAttribute('aria-disabled', 'true');
  await dialog.getByRole('button', { name: 'Abbrechen' }).click();
  await expect(dialog).toBeHidden();
  await expect(
    row(page, 'kunden-portal').getByLabel('Freigabe: Bypass'),
  ).toBeVisible();
});
