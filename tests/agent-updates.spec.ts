import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { AppSettings, Snapshot } from '../src/lib/types';

/** Settings → „Updates“, where the „Agenten“ group sits below the app update. */
async function openUpdates(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await dialog.getByRole('tab', { name: 'Updates', exact: true }).click();
  return dialog;
}
const autoUpdate = (dialog: ReturnType<Page['getByRole']>) =>
  dialog.getByRole('checkbox', { name: 'Automatisch aktualisieren' });
const saveButton = (dialog: ReturnType<Page['getByRole']>) =>
  dialog.getByRole('button', { name: 'Speichern', exact: true });

test('„Automatisch aktualisieren“ is staged: „Abbrechen“ drops it, „Speichern“ keeps it', async ({
  page,
}) => {
  await page.goto('/');
  let dialog = await openUpdates(page);
  const group = dialog.getByRole('region', { name: 'Agenten' });
  await expect(group).toContainText('Alle 4 Stunden, nur ohne offene Session.');
  await expect(autoUpdate(dialog)).not.toBeChecked();
  await expect(saveButton(dialog)).toHaveAttribute('aria-disabled', 'true');
  await dialog.getByText('Automatisch aktualisieren').click();
  await expect(autoUpdate(dialog)).toBeChecked();
  await expect(saveButton(dialog)).not.toHaveAttribute('aria-disabled');
  await dialog.getByRole('button', { name: 'Abbrechen', exact: true }).click();
  await expect(dialog).toHaveCount(0);

  dialog = await openUpdates(page);
  await expect(autoUpdate(dialog)).not.toBeChecked();
  await autoUpdate(dialog).check();
  await saveButton(dialog).click();
  await expect(dialog).toHaveCount(0);
  dialog = await openUpdates(page);
  await expect(autoUpdate(dialog)).toBeChecked();
});

test('„Jetzt aktualisieren“ shows a running row per agent that settles', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openUpdates(page);
  const list = dialog.getByRole('list', { name: 'Agenten-Updates' });
  await expect(list).toHaveCount(0);
  const button = dialog.getByRole('button', { name: 'Jetzt aktualisieren' });
  await button.click();
  const rows = list.getByRole('listitem');
  await expect(rows).toHaveCount(3);
  await expect(rows.first()).toContainText('wird aktualisiert …');
  await expect(button).toHaveAttribute('aria-disabled', 'true');

  await expect(rows.nth(0)).toContainText('Claude Code');
  await expect(rows.nth(0)).toContainText('aktualisiert auf 2.1.318');
  await expect(
    rows.nth(0).getByText('aktualisiert auf 2.1.318'),
  ).toHaveAttribute('title', '2.1.317 → 2.1.318');
  await expect(rows.nth(1)).toContainText('OpenCode');
  await expect(rows.nth(1)).toContainText('in Benutzung – übersprungen');
  await expect(rows.nth(2)).toContainText('Codex');
  await expect(rows.nth(2)).toContainText('aktuell · 0.166.0');
  await expect(rows.nth(2)).toContainText('gerade eben');
  await expect(button).not.toHaveAttribute('aria-disabled');
  // An immediate action stages nothing.
  await expect(saveButton(dialog)).toHaveAttribute('aria-disabled', 'true');
});

test('natively the setting saves with the settings and a refused run shows its error', async ({
  page,
}) => {
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => {
      let current = snapshot;
      return async (cmd, args) => {
        if (cmd === 'snapshot') return current;
        if (cmd === 'save_settings') {
          current = { ...current, settings: args?.settings as AppSettings };
          return current;
        }
        if (cmd === 'update_agents')
          throw 'Agenten werden bereits aktualisiert.';
        if (cmd === 'autostart_status')
          return { status: 'disabled', registered: false };
        return null;
      };
    },
    { arg: nativeSnapshot(), state: {} },
  );
  await page.goto('/');
  const dialog = await openUpdates(page);
  await dialog.getByRole('button', { name: 'Jetzt aktualisieren' }).click();
  await expect(
    dialog.getByRole('region', { name: 'Agenten' }).getByRole('alert'),
  ).toHaveText('Agenten werden bereits aktualisiert.');
  expect(
    (await readCalls(page, 'update_agents')).map((call) => call.cmd),
  ).toEqual(['update_agents']);

  await autoUpdate(dialog).check();
  await saveButton(dialog).click();
  await expect(dialog).toHaveCount(0);
  const expected = nativeSnapshot().settings;
  expected.agentUpdates = true;
  expect(
    (await readCalls(page, 'save_settings')).map((call) => call.args),
  ).toEqual([{ settings: expected }]);
});
