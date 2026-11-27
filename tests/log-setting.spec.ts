import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { Snapshot } from '../src/lib/types';

/*
 * „Protokoll“ in Settings → App (#109): the level is staged with the other
 * settings and saved by „Speichern“; „Ordner öffnen“ acts at once and exists
 * only natively. In the labelled demo and against the labelled native-IPC
 * mock (synthetic data only).
 */
const dialog = (page: Page) =>
  page.getByRole('dialog', { name: 'Einstellungen', exact: true });
const save = (page: Page) =>
  dialog(page).getByRole('button', { name: 'Speichern', exact: true });
const level = (page: Page, name: string) =>
  dialog(page)
    .getByRole('radiogroup', { name: 'Protokoll' })
    .getByRole('radio', { name, exact: true });
const folder = (page: Page) =>
  dialog(page).getByRole('button', { name: 'Ordner öffnen', exact: true });
async function openApp(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  await dialog(page).getByRole('tab', { name: /^App/ }).click();
}

test('the demo stages the level until „Speichern“ and has no folder', async ({
  page,
}) => {
  await page.goto('/');
  await openApp(page);
  await expect(level(page, 'Info')).toBeChecked();
  await expect(level(page, 'Debug')).not.toBeChecked();
  // The browser demo writes no log file.
  await expect(folder(page)).toHaveCount(0);
  await expect(save(page)).toBeDisabled();
  // A staged level is dropped by „Abbrechen“.
  await level(page, 'Debug').check();
  await expect(save(page)).toBeEnabled();
  await dialog(page)
    .getByRole('button', { name: 'Abbrechen', exact: true })
    .click();
  await openApp(page);
  await expect(level(page, 'Info')).toBeChecked();
  await level(page, 'Debug').check();
  await save(page).click();
  await expect(dialog(page)).toHaveCount(0);
  await openApp(page);
  await expect(level(page, 'Debug')).toBeChecked();
  await expect(save(page)).toBeDisabled();
});

test('natively „Speichern“ sends `logLevel` and „Ordner öffnen“ opens the folder at once', async ({
  page,
}) => {
  const mock = await installTauriMock<Snapshot, { failOpen: boolean }>(
    page,
    (snapshot, state) => {
      let current = structuredClone(snapshot);
      return (cmd, args) => {
        if (cmd === 'snapshot' || cmd === 'refresh_detection')
          return structuredClone(current);
        if (cmd === 'autostart_status')
          return { status: 'disabled', registered: false };
        if (cmd === 'save_settings') {
          const settings = args?.settings as Snapshot['settings'];
          current = { ...current, settings, revision: current.revision + 1 };
          return structuredClone(current);
        }
        if (cmd === 'open_log_folder' && state.failOpen)
          throw 'Log-Ordner konnte nicht geöffnet werden.';
        return null;
      };
    },
    {
      arg: nativeSnapshot(),
      state: { failOpen: false },
    },
  );
  await page.goto('/');
  await openApp(page);
  await expect(level(page, 'Info')).toBeChecked();
  // The folder button acts at once and stages nothing.
  await folder(page).click();
  await expect
    .poll(async () => (await readCalls(page, 'open_log_folder')).length)
    .toBe(1);
  await expect(save(page)).toBeDisabled();
  await expect(dialog(page)).toBeVisible();
  await level(page, 'Debug').check();
  expect(await readCalls(page, 'save_settings')).toEqual([]);
  await save(page).click();
  await expect(dialog(page)).toHaveCount(0);
  const saved = await readCalls(page, 'save_settings');
  expect(saved).toHaveLength(1);
  expect(saved[0]?.args?.settings).toMatchObject({ logLevel: 'debug' });
  // A folder that cannot be opened is reported in the section and stays.
  await mock.configure({ failOpen: true });
  await openApp(page);
  await expect(level(page, 'Debug')).toBeChecked();
  await folder(page).click();
  await expect(
    dialog(page).locator('.log-setting').getByRole('alert'),
  ).toHaveText('Log-Ordner konnte nicht geöffnet werden.');
});
