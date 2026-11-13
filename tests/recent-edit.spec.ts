import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import { expect, test, type Page } from './support/test';
import type { Snapshot } from '../src/lib/types';

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

test('changes a Lokal row’s profile; a Docker row has none', async ({
  page,
}) => {
  await page.goto('/');
  const target = row(page, 'traygent');
  const mark = target.getByRole('img', { name: 'Profil Team' });
  await expect(mark).toHaveCount(0);
  const dialog = await openEdit(page, 'traygent');
  const field = dialog.getByRole('combobox', { name: 'Profil' });
  await expect(field).toHaveText('Standard');
  await field.click();
  const list = dialog.getByRole('listbox', { name: 'Profil' });
  await expect(list.getByRole('option')).toHaveText(['Standard', 'Team']);
  await list.getByRole('option', { name: 'Team' }).click();
  await dialog.getByRole('button', { name: 'Speichern' }).click();
  await expect(dialog).toBeHidden();
  await expect(mark).toBeVisible();
  // Back to „Standard“: the mark goes.
  const again = await openEdit(page, 'traygent');
  await expect(again.getByRole('combobox', { name: 'Profil' })).toHaveText(
    'Team',
  );
  await again.getByRole('combobox', { name: 'Profil' }).click();
  await again.getByRole('option', { name: 'Standard' }).click();
  await again.getByRole('button', { name: 'Speichern' }).click();
  await expect(again).toBeHidden();
  await expect(mark).toHaveCount(0);
  const docker = await openEdit(page, 'kunden-portal');
  await expect(docker.getByRole('combobox', { name: 'Profil' })).toHaveCount(0);
});

test('natively: a profile removed since shows and saves as „Standard“', async ({
  page,
}) => {
  const snapshot = nativeSnapshot();
  const notes = snapshot.recents.find((r) => r.folder.endsWith('Notizen'))!;
  notes.profile = 'gone';
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd) =>
      ['snapshot', 'refresh_detection', 'update_recent'].includes(cmd)
        ? snapshot
        : null,
    { arg: snapshot, state: {} },
  );
  await page.goto('/');
  const dialog = await openEdit(page, 'Notizen');
  await expect(dialog.getByRole('combobox', { name: 'Profil' })).toHaveText(
    'Standard',
  );
  // What „Speichern“ would store differs from the row: it is enabled.
  const save = dialog.getByRole('button', { name: 'Speichern' });
  await expect(save).not.toHaveAttribute('aria-disabled');
  await save.click();
  await expect(dialog).toBeHidden();
  const [call] = await readCalls(page, 'update_recent');
  expect(call?.args).toEqual({
    id: notes.id,
    options: notes.options,
    ports: [],
    profile: null,
  });
});
