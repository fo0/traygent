import { choose } from './support/dropdown';
import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { AppSettings, Snapshot } from '../src/lib/types';

async function openSettings(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  return page.getByRole('dialog', { name: 'Einstellungen' });
}
const saveButton = (page: Page) =>
  page
    .getByRole('dialog', { name: 'Einstellungen' })
    .getByRole('button', { name: 'Speichern', exact: true });

test('Agenten: staged defaults apply with „Speichern“ and reach the composer', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openSettings(page);
  await expect(
    dialog.getByRole('radio', { name: 'Claude Code' }),
  ).toBeChecked();
  await expect(dialog.getByText('2.1.317')).toBeVisible();
  await expect(saveButton(page)).toHaveAttribute('aria-disabled', 'true');
  const permission = dialog.getByRole('radiogroup', { name: 'Freigabe' });
  await permission.getByRole('radio', { name: 'Bypass' }).check();
  await choose(dialog.getByRole('combobox', { name: 'Denken' }), 'Maximal');
  await expect(saveButton(page)).not.toHaveAttribute('aria-disabled');
  await saveButton(page).click();
  await expect(dialog).toHaveCount(0);

  const chip = page
    .locator('form.composer')
    .getByRole('button', { name: /^Freigabe: / });
  await expect(chip).toHaveAccessibleName('Freigabe: Bypass');
  await expect(
    page.locator('form.composer').getByRole('combobox', { name: 'Denken' }),
  ).toHaveAttribute('data-value', 'max');

  const reopened = await openSettings(page);
  await expect(
    reopened
      .getByRole('radiogroup', { name: 'Freigabe' })
      .getByRole('radio', { name: 'Bypass' }),
  ).toBeChecked();
});

test('„Abbrechen“ drops staged agent defaults', async ({ page }) => {
  await page.goto('/');
  let dialog = await openSettings(page);
  await dialog.getByRole('radio', { name: 'Codex' }).check();
  await dialog
    .getByRole('textbox', { name: 'Extra-Argumente' })
    .fill('--search');
  await dialog.getByRole('button', { name: 'Abbrechen', exact: true }).click();
  dialog = await openSettings(page);
  await dialog.getByRole('radio', { name: 'Codex' }).check();
  await expect(
    dialog.getByRole('textbox', { name: 'Extra-Argumente' }),
  ).toHaveValue('');
});

test('invalid extra arguments block „Speichern“ and mark their tab', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openSettings(page);
  const args = dialog.getByRole('textbox', { name: 'Extra-Argumente' });
  await args.fill('--name "x"');
  await expect(args).toHaveAttribute('aria-invalid', 'true');
  await expect(args).toHaveAccessibleDescription('Keine Anführungszeichen.');
  await expect(saveButton(page)).toHaveAttribute('aria-disabled', 'true');
  // The tab's dot names the problem; a tab's children are presentational.
  await expect(
    dialog.getByRole('tab', { name: /^Agenten/ }).locator('.tab-update-dot'),
  ).toHaveAttribute('aria-label', 'Ungültige Eingabe');
  await saveButton(page).click({ force: true });
  await expect(dialog).toBeVisible();
  await args.fill('--verbose');
  await expect(saveButton(page)).not.toHaveAttribute('aria-disabled');
});

test('Umgebung: default environment, terminal and Docker image save together', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openSettings(page);
  await dialog.getByRole('tab', { name: 'Umgebung', exact: true }).click();
  const env = dialog.getByRole('radiogroup', { name: 'Standard-Umgebung' });
  await expect(env.getByRole('radio', { name: 'WSL' })).toBeDisabled();
  await env.getByRole('radio', { name: 'Docker' }).check();
  await dialog
    .getByRole('radiogroup', { name: 'Terminal' })
    .getByRole('radio', { name: 'Konsole' })
    .check();
  const image = dialog.getByRole('textbox', { name: 'Codex' });
  await image.fill('');
  await expect(image).toHaveAccessibleDescription('Image fehlt.');
  await expect(saveButton(page)).toHaveAttribute('aria-disabled', 'true');
  await image.fill('node:24-trixie');
  const token = dialog.getByRole('checkbox', {
    name: 'GitHub-Token weitergeben',
  });
  await dialog.getByText('GitHub-Token weitergeben').click();
  await expect(token).toBeChecked();
  await saveButton(page).click();
  await expect(dialog).toHaveCount(0);
  await expect(
    page.locator('form.composer').getByRole('radio', { name: 'Docker' }),
  ).toBeChecked();
});

test('Umgebung: hidden environments leave the composer; the last one stays on', async ({
  page,
}) => {
  await page.goto('/');
  const composer = page.locator('form.composer');
  await expect(
    composer.getByRole('radiogroup', { name: 'Umgebung' }),
  ).toBeVisible();
  const dialog = await openSettings(page);
  await dialog.getByRole('tab', { name: 'Umgebung', exact: true }).click();
  const shown = dialog.getByRole('group', { name: 'Umgebungen' });
  const local = shown.getByRole('checkbox', { name: 'Lokal' });
  await expect(local).toBeEnabled();
  await shown.getByRole('checkbox', { name: 'Docker' }).uncheck();
  await shown.getByRole('checkbox', { name: 'WSL' }).uncheck();
  // Lokal is the last one: locked on, still focusable, the tooltip says why.
  await expect(local).toBeChecked();
  await expect(local).toHaveAttribute('aria-disabled', 'true');
  await expect(local).toHaveAccessibleDescription(
    'Mindestens eine Umgebung muss aktiv sein.',
  );
  await local.focus();
  await expect(local).toBeFocused();
  await page.keyboard.press('Space');
  await local.click({ force: true });
  await shown.getByText('Lokal', { exact: true }).click({ force: true });
  await expect(local).toBeChecked();
  // A single environment needs no default row.
  await expect(
    dialog.getByRole('radiogroup', { name: 'Standard-Umgebung' }),
  ).toHaveCount(0);
  await saveButton(page).click();
  await expect(dialog).toHaveCount(0);
  await expect(
    composer.getByRole('radiogroup', { name: 'Umgebung' }),
  ).toHaveCount(0);
  await expect(page.locator('.docker-state')).toHaveCount(0);

  // Hiding is a display preference: a Docker recent still restarts as it was.
  const row = page.locator('.recent-row').filter({ hasText: 'kunden-portal' });
  await expect(row.getByRole('img', { name: 'Docker' })).toBeVisible();
  await row
    .getByRole('button', { name: 'Claude Code in kunden-portal starten' })
    .click();
  await expect(
    page.getByRole('status').filter({ hasText: 'gestartet (Demo).' }),
  ).toBeVisible();
  await expect(row.getByRole('img', { name: 'Docker' })).toBeVisible();
});

test('a hidden Docker takes its settings and probes along; its invalid fields reset', async ({
  page,
}) => {
  await page.goto('/');
  let dialog = await openSettings(page);
  const status = dialog.locator('.status-lines');
  await expect(status.locator('dt', { hasText: 'Docker' })).toBeVisible();
  await dialog.getByRole('tab', { name: 'Umgebung', exact: true }).click();
  await dialog
    .getByRole('textbox', { name: 'Claude Code' })
    .fill('node:24-trixie');
  await dialog.getByRole('textbox', { name: 'Codex' }).fill('');
  await expect(saveButton(page)).toHaveAttribute('aria-disabled', 'true');
  const dockerToggle = (scope: typeof dialog) =>
    scope
      .getByRole('group', { name: 'Umgebungen' })
      .getByRole('checkbox', { name: 'Docker' });
  await dockerToggle(dialog).uncheck();
  // Hidden at once; the invalid image went back to its saved value.
  await expect(dialog.getByRole('heading', { name: 'Docker' })).toHaveCount(0);
  await expect(
    dialog.getByRole('group', { name: 'Standard-Ports' }),
  ).toHaveCount(0);
  await expect(saveButton(page)).not.toHaveAttribute('aria-disabled');
  await dialog.getByRole('tab', { name: /^Agenten/ }).click();
  await expect(status.locator('dt', { hasText: 'Lokal' })).toBeVisible();
  await expect(status.locator('dt', { hasText: 'Docker' })).toHaveCount(0);
  await saveButton(page).click();
  await expect(dialog).toHaveCount(0);

  // Ticked again: the saved image, and the valid edit that was staged.
  dialog = await openSettings(page);
  await dialog.getByRole('tab', { name: 'Umgebung', exact: true }).click();
  await expect(dialog.getByRole('heading', { name: 'Docker' })).toHaveCount(0);
  await dockerToggle(dialog).check();
  await expect(dialog.getByRole('textbox', { name: 'Codex' })).toHaveValue(
    'node:22-trixie',
  );
  await expect(
    dialog.getByRole('textbox', { name: 'Claude Code' }),
  ).toHaveValue('node:24-trixie');
});

test('„Installieren“ is immediate; the demo opens no terminal and says so briefly', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openSettings(page);
  await dialog.getByRole('radio', { name: 'OpenCode' }).check();
  await expect(
    dialog.getByText('Nicht installiert', { exact: true }),
  ).toBeVisible();
  await dialog
    .getByRole('button', { name: 'Installieren', exact: true })
    .click();
  const note = dialog.getByRole('status').filter({
    hasText: 'Demo: kein Terminal geöffnet.',
  });
  await expect(note).toBeVisible();
  await expect(note).toHaveCount(0, { timeout: 6000 });
  // Nothing was staged by it.
  await expect(saveButton(page)).toHaveAttribute('aria-disabled', 'true');
});

/** The labelled native-IPC mock: `save_settings` answers with what it got. */
const nativeSettings = (page: Page) =>
  installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => {
      let current = snapshot;
      return async (cmd, args) => {
        if (cmd === 'snapshot' || cmd === 'refresh_detection') return current;
        if (cmd === 'save_settings') {
          current = { ...current, settings: args?.settings as AppSettings };
          return current;
        }
        if (cmd === 'autostart_status')
          return { status: 'disabled', registered: false };
        return null;
      };
    },
    { arg: nativeSnapshot(), state: {} },
  );

test('natively „Speichern“ sends the whole settings object once', async ({
  page,
}) => {
  await nativeSettings(page);
  await page.goto('/');
  const dialog = await openSettings(page);
  await dialog.getByRole('tab', { name: 'Umgebung', exact: true }).click();
  await dialog
    .getByRole('radiogroup', { name: 'Standard-Agent' })
    .getByRole('radio', { name: 'Codex' })
    .check();
  await saveButton(page).click();
  await expect(dialog).toHaveCount(0);
  const calls = await readCalls(page, 'save_settings');
  const expected = nativeSnapshot().settings;
  expected.defaultAgent = 'codex';
  expect(calls.map((call) => call.args)).toEqual([{ settings: expected }]);
  await expect(
    page.locator('form.composer').getByRole('radio', { name: 'Codex' }),
  ).toBeChecked();
});

test('„Ein Fenster für alle Sessions“: Windows Terminal only, staged until „Speichern“', async ({
  page,
}) => {
  await nativeSettings(page);
  await page.goto('/');
  const dialog = await openSettings(page);
  await dialog.getByRole('tab', { name: 'Umgebung', exact: true }).click();
  const shared = dialog.getByRole('checkbox', {
    name: 'Ein Fenster für alle Sessions',
  });
  const terminal = dialog.getByRole('radiogroup', { name: 'Terminal' });
  await expect(shared).not.toBeChecked();
  await expect(
    dialog.locator('label.switch-row', {
      hasText: 'Ein Fenster für alle Sessions',
    }),
  ).toHaveAttribute(
    'title',
    'Jede Session öffnet als neuer Tab im selben Windows-Terminal-Fenster',
  );
  await terminal.getByRole('radio', { name: 'Konsole' }).check();
  await expect(shared).toHaveCount(0);
  await terminal.getByRole('radio', { name: 'Windows Terminal' }).check();
  await dialog.getByText('Ein Fenster für alle Sessions').click();
  await expect(shared).toBeChecked();
  expect(await readCalls(page, 'save_settings')).toEqual([]);
  await saveButton(page).click();
  await expect(dialog).toHaveCount(0);
  const expected = nativeSnapshot().settings;
  expected.sharedWindow = true;
  expect(
    (await readCalls(page, 'save_settings')).map((call) => call.args),
  ).toEqual([{ settings: expected }]);
});
