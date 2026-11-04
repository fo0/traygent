import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { LaunchRequest, Snapshot } from '../src/lib/types';

/*
 * Ports of Docker sessions (docs/SPEC.md → Ports): the composer's „Ports“
 * chip, the default list in Einstellungen → Umgebung and the restart of a
 * recent session that published ports.
 */
const composer = (page: Page) => page.locator('form.composer');
const portsChip = (page: Page) =>
  composer(page).getByRole('button', { name: /^Ports( \d+)?$/ });
const popover = (page: Page) => page.getByRole('dialog', { name: 'Ports' });
const start = (page: Page) =>
  composer(page).getByRole('button', { name: 'Starten', exact: true });

/** The labelled native mock: answers a start with the request recorded. */
async function nativeLaunches(page: Page) {
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd, args) => {
      if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
      if (cmd === 'launch_session') {
        const request = args?.request as LaunchRequest;
        const recent = {
          ...snapshot.recents[0]!,
          env: request.env,
          ports: request.ports,
          lastStartedAt: new Date().toISOString(),
        };
        return {
          recent,
          snapshot: {
            ...snapshot,
            revision: snapshot.revision + 1,
            recents: [recent, ...snapshot.recents.slice(1)],
          },
        };
      }
      return null;
    },
    { arg: nativeSnapshot(), state: {} },
  );
}
const launchedPorts = async (page: Page) =>
  (await readCalls(page, 'launch_session')).map(
    (call) => (call.args?.request as LaunchRequest).ports,
  );

test('the „Ports“ chip shows for Docker only', async ({ page }) => {
  await page.goto('/');
  const form = composer(page);
  await expect(form.getByRole('radio', { name: 'Lokal' })).toBeChecked();
  await expect(portsChip(page)).toHaveCount(0);
  await form.getByRole('radio', { name: 'Docker' }).check();
  // A symbol only: the word is its name, the choice its tooltip.
  await expect(portsChip(page)).toHaveText('');
  await expect(portsChip(page)).toHaveAccessibleName('Ports');
  await expect(portsChip(page)).toHaveAttribute('title', 'Ports: keine');
  await form.getByRole('radio', { name: 'Lokal' }).check();
  await expect(portsChip(page)).toHaveCount(0);
});

test('presets and a custom port reach launch_session normalized', async ({
  page,
}) => {
  await nativeLaunches(page);
  await page.goto('/');
  await page
    .getByRole('combobox', { name: 'Ordner oder Repo' })
    .fill('C:\\Users\\demo\\Projekte\\traygent');
  await composer(page).getByRole('radio', { name: 'Docker' }).check();
  await portsChip(page).click();
  const picker = popover(page);
  await expect(picker.getByRole('group', { name: 'Ports' })).toHaveAttribute(
    'title',
    'Dienst im Container auf 0.0.0.0 binden',
  );
  // The first checkbox has focus; the presets follow the snapshot's order.
  const first = picker.getByRole('checkbox', { name: '3000 Next.js · Node' });
  await expect(first).toBeFocused();
  await expect(picker.getByRole('checkbox')).toHaveCount(8);
  await first.check();
  await picker.getByRole('checkbox', { name: '5173 Vite' }).check();
  const field = picker.getByRole('textbox', { name: 'Eigener Port' });
  await field.fill('abc');
  await field.press('Enter');
  await expect(field).toHaveAttribute('aria-invalid', 'true');
  await expect(field).toHaveAccessibleDescription('Port „abc“ ist ungültig.');
  // Enter adds here, it never starts the session.
  expect(await launchedPorts(page)).toEqual([]);
  await field.fill('3000:80');
  await field.press('Enter');
  await expect(field).toHaveAccessibleDescription(
    'Host-Port 3000 ist doppelt belegt.',
  );
  await field.fill('8080:80/udp');
  await expect(field).not.toHaveAttribute('aria-invalid');
  await picker.getByRole('button', { name: 'Port hinzufügen' }).click();
  await expect(field).toHaveValue('');
  await expect(picker.locator('.port-chip')).toHaveText([
    '127.0.0.1:8080:80/udp',
  ]);
  await picker.getByRole('checkbox', { name: '5173 Vite' }).uncheck();
  await expect(portsChip(page)).toHaveText('2');
  await expect(portsChip(page)).toHaveAccessibleName('Ports 2');
  // An outside click closes the popover; the choice stays.
  await page.locator('.app-bar').click({ position: { x: 240, y: 20 } });
  await expect(picker).toHaveCount(0);
  await expect(portsChip(page)).toHaveAttribute(
    'title',
    'Ports: 127.0.0.1:3000:3000, 127.0.0.1:8080:80/udp',
  );

  await start(page).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Claude Code gestartet.' }),
  ).toBeVisible();
  expect(await launchedPorts(page)).toEqual([
    ['127.0.0.1:3000:3000', '127.0.0.1:8080:80/udp'],
  ]);
  // The next start begins from the saved defaults again.
  await expect(portsChip(page)).toHaveAccessibleName('Ports');
  // Lokal never sends ports.
  await composer(page).getByRole('radio', { name: 'Lokal' }).check();
  await start(page).click();
  await expect.poll(() => launchedPorts(page)).toHaveLength(2);
  expect((await launchedPorts(page))[1]).toEqual([]);
});

test('the keyboard opens, toggles and closes the picker', async ({ page }) => {
  await page.goto('/');
  await composer(page).getByRole('radio', { name: 'Docker' }).check();
  const chip = portsChip(page);
  await chip.focus();
  await expect(chip).toHaveAttribute('aria-expanded', 'false');
  await page.keyboard.press('Enter');
  await expect(chip).toHaveAttribute('aria-expanded', 'true');
  const first = popover(page).getByRole('checkbox').first();
  await expect(first).toBeFocused();
  await page.keyboard.press('Space');
  await expect(first).toBeChecked();
  await page.keyboard.press('Escape');
  await expect(popover(page)).toHaveCount(0);
  await expect(chip).toBeFocused();
  await expect(chip).toHaveAccessibleName('Ports 1');
  // The custom field: typed, added with Enter, removed by its chip button.
  await page.keyboard.press('Enter');
  const field = popover(page).getByRole('textbox', { name: 'Eigener Port' });
  await field.focus();
  await page.keyboard.type('6000-6001');
  await page.keyboard.press('Enter');
  const remove = popover(page).getByRole('button', {
    name: '127.0.0.1:6000-6001:6000-6001 entfernen',
  });
  await remove.focus();
  await page.keyboard.press('Enter');
  await expect(remove).toHaveCount(0);
  await expect(field).toBeFocused();
  await expect(popover(page)).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(chip).toBeFocused();
  await expect(chip).toHaveAccessibleName('Ports 1');
});

test('Einstellungen → Umgebung saves the default ports', async ({ page }) => {
  await page.goto('/');
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  let dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await dialog.getByRole('tab', { name: 'Umgebung', exact: true }).click();
  const defaults = dialog.getByRole('group', { name: 'Standard-Ports' });
  await defaults.getByRole('checkbox', { name: '5173 Vite' }).check();
  await defaults.getByRole('textbox', { name: 'Eigener Port' }).fill('9000');
  await defaults.getByRole('textbox', { name: 'Eigener Port' }).press('Enter');
  // Enter in the field adds the port; the dialog stays open, staged.
  await expect(dialog).toBeVisible();
  await expect(defaults.locator('.port-chip')).toHaveText([
    '127.0.0.1:9000:9000',
  ]);
  await dialog.getByRole('button', { name: 'Speichern', exact: true }).click();
  await expect(dialog).toHaveCount(0);

  await composer(page).getByRole('radio', { name: 'Docker' }).check();
  await expect(portsChip(page)).toHaveAccessibleName('Ports 2');
  await expect(portsChip(page)).toHaveAttribute(
    'title',
    'Ports: 127.0.0.1:5173:5173, 127.0.0.1:9000:9000',
  );
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await dialog.getByRole('tab', { name: 'Umgebung', exact: true }).click();
  await expect(
    dialog
      .getByRole('group', { name: 'Standard-Ports' })
      .getByRole('checkbox', { name: '5173 Vite' }),
  ).toBeChecked();
});

test('a duplicate default port blocks „Speichern“ and marks the tab', async ({
  page,
}) => {
  await page.goto('/');
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await dialog.getByRole('tab', { name: 'Umgebung', exact: true }).click();
  const defaults = dialog.getByRole('group', { name: 'Standard-Ports' });
  const field = defaults.getByRole('textbox', { name: 'Eigener Port' });
  await field.fill('3000:80');
  await field.press('Enter');
  await defaults.getByRole('checkbox', { name: '3000 Next.js · Node' }).check();
  await expect(defaults.locator('.field-error')).toHaveText(
    'Host-Port 3000 ist doppelt belegt.',
  );
  await expect(
    dialog.getByRole('button', { name: 'Speichern', exact: true }),
  ).toHaveAttribute('aria-disabled', 'true');
  await expect(
    dialog.getByRole('tab', { name: /^Umgebung/ }).locator('.tab-update-dot'),
  ).toHaveAttribute('aria-label', 'Ungültige Eingabe');
});

test('▶ restarts a recent Docker session with exactly its ports', async ({
  page,
}) => {
  await nativeLaunches(page);
  await page.goto('/');
  const row = page.locator('.recent-row').filter({
    has: page.locator('.recent-name', { hasText: 'kunden-portal' }),
  });
  const chip = row.getByRole('img', {
    name: 'Ports: 127.0.0.1:3000:3000, 127.0.0.1:5173:5173',
  });
  await expect(chip).toHaveText('3000 +1');
  await row
    .getByRole('button', { name: 'Claude Code in kunden-portal starten' })
    .click();
  await expect
    .poll(() => launchedPorts(page))
    .toEqual([['127.0.0.1:3000:3000', '127.0.0.1:5173:5173']]);
  // Rows without ports show no chip and restart with none.
  const local = page
    .locator('.recent-row')
    .filter({ has: page.locator('.recent-name', { hasText: 'repo-watch' }) });
  await expect(local.locator('.recent-ports')).toHaveCount(0);
  await local
    .getByRole('button', { name: 'Codex in repo-watch starten' })
    .click();
  await expect.poll(() => launchedPorts(page)).toHaveLength(2);
  expect((await launchedPorts(page))[1]).toEqual([]);
});
