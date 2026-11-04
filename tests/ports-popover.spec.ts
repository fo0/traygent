import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { Snapshot } from '../src/lib/types';

/*
 * The composer's „Ports“ popover (docs/SPEC.md → Ports): keys and focus stay
 * inside it, it fits the content area, and a binding the network reaches
 * (any IP but 127.0.0.1) shows its IP in the warning style.
 */
const composer = (page: Page) => page.locator('form.composer');
const portsChip = (page: Page) =>
  composer(page).getByRole('button', { name: /^Ports\b/ });
const popover = (page: Page) => page.getByRole('dialog', { name: 'Ports' });

async function openPicker(page: Page) {
  await page.goto('/');
  await composer(page).getByRole('radio', { name: 'Docker' }).check();
  await portsChip(page).click();
  await expect(popover(page)).toBeVisible();
}

/** The labelled native mock: any start is recorded, never run. */
async function nativeWith(page: Page, snapshot: Snapshot) {
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (arg) => async (cmd) =>
      cmd === 'snapshot' || cmd === 'refresh_detection' ? arg : null,
    { arg: snapshot, state: {} },
  );
}

test('Enter on a preset checks nothing and starts no session', async ({
  page,
}) => {
  await nativeWith(page, nativeSnapshot());
  await openPicker(page);
  const first = popover(page).getByRole('checkbox').first();
  await expect(first).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(popover(page)).toBeVisible();
  await expect(first).toBeFocused();
  await expect(first).not.toBeChecked();
  expect(await readCalls(page, 'launch_session')).toEqual([]);
  // The custom field still adds with Enter.
  const field = popover(page).getByRole('textbox', { name: 'Eigener Port' });
  await field.fill('9000');
  await field.press('Enter');
  await expect(popover(page).locator('.port-chip')).toHaveText([
    '127.0.0.1:9000:9000',
  ]);
  expect(await readCalls(page, 'launch_session')).toEqual([]);
});

test('Escape on the chip closes the open popover', async ({ page }) => {
  await openPicker(page);
  await portsChip(page).focus();
  await expect(popover(page)).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(popover(page)).toHaveCount(0);
  await expect(portsChip(page)).toBeFocused();
  await expect(portsChip(page)).toHaveAttribute('aria-expanded', 'false');
});

test('a click on padding or the error line keeps focus in the field', async ({
  page,
}) => {
  await openPicker(page);
  const field = popover(page).getByRole('textbox', { name: 'Eigener Port' });
  await field.fill('abc');
  await field.press('Enter');
  const error = popover(page).locator('.field-error');
  await expect(error).toHaveText('Port „abc“ ist ungültig.');
  await error.click();
  await expect(field).toBeFocused();
  await popover(page).click({ position: { x: 3, y: 3 } });
  await expect(field).toBeFocused();
  await expect(popover(page)).toBeVisible();
});

test('the popover stays inside the content area at 360×560', async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 560 });
  await openPicker(page);
  const field = popover(page).getByRole('textbox', { name: 'Eigener Port' });
  /** How far the popover stays inside the content area, top and bottom. */
  const room = async () => {
    const box = (await popover(page).boundingBox())!;
    const content = (await page.locator('.content').boundingBox())!;
    return Math.min(
      box.y - content.y,
      content.y + content.height - (box.y + box.height),
    );
  };
  for (const entry of ['9000', '9100:80', '9200-9201/udp']) {
    await field.fill(entry);
    await field.press('Enter');
  }
  await expect(popover(page).locator('.port-chip')).toHaveCount(3);
  expect(await room()).toBeGreaterThanOrEqual(0);
  await expect(field).toBeInViewport();
  // The exposed IP lengthens the chip label and wraps the chip row after
  // the change; the popover is placed again from the reflowed row.
  await popover(page)
    .getByRole('button', { name: '127.0.0.1:9100:80 entfernen' })
    .click();
  await field.fill('0.0.0.0:7000:7000');
  await field.press('Enter');
  await expect(portsChip(page)).toHaveText('3 · 0.0.0.0');
  await expect(popover(page).locator('.port-chip')).toHaveCount(3);
  await expect.poll(room).toBeGreaterThanOrEqual(0);
  await expect(field).toBeInViewport();
});

test('a port the network reaches shows its IP as a warning', async ({
  page,
}) => {
  const snapshot = nativeSnapshot();
  snapshot.recents = snapshot.recents.map((recent) =>
    recent.folder.endsWith('kunden-portal')
      ? { ...recent, ports: ['127.0.0.1:5173:5173', '0.0.0.0:3000:3000'] }
      : recent,
  );
  await nativeWith(page, snapshot);
  await page.setViewportSize({ width: 360, height: 640 });
  await page.goto('/');
  const row = page.locator('.recent-row').filter({
    has: page.locator('.recent-name', { hasText: 'kunden-portal' }),
  });
  const recentChip = row.getByRole('img', {
    name: 'Ports: 127.0.0.1:5173:5173, 0.0.0.0:3000:3000 · im Netzwerk erreichbar',
  });
  // Narrow rows keep the IP in view; loopback-only chips shrink to the icon.
  await expect(recentChip).toHaveText('0.0.0.0:3000 +1');
  await expect(recentChip).toBeVisible();
  await expect(recentChip.locator('span')).toBeVisible();
  await expect(recentChip).toHaveAttribute('data-warn', 'true');

  await row.getByRole('button', { name: 'Aktionen für kunden-portal' }).click();
  await page.getByRole('menuitem', { name: 'In Composer übernehmen' }).click();
  const chip = portsChip(page);
  await expect(chip).toHaveText('2 · 0.0.0.0');
  await expect(chip).toHaveAccessibleName(
    'Ports 2 · 0.0.0.0, im Netzwerk erreichbar',
  );
  await expect(chip).toHaveAttribute('data-warn', 'true');
  await expect(chip).toHaveAttribute(
    'title',
    'Ports: 127.0.0.1:5173:5173, 0.0.0.0:3000:3000 · im Netzwerk erreichbar',
  );
  await chip.click();
  const custom = popover(page).locator('.port-chip');
  await expect(custom).toHaveCount(1);
  await expect(custom).toHaveAttribute('data-warn', 'true');
  await expect(custom).toHaveAttribute(
    'title',
    '0.0.0.0:3000:3000 · im Netzwerk erreichbar',
  );
  await expect(custom.locator('.mono')).toHaveText('0.0.0.0:3000:3000');
  // Loopback entries keep the plain style.
  await popover(page)
    .getByRole('textbox', { name: 'Eigener Port' })
    .fill('9000');
  await popover(page)
    .getByRole('textbox', { name: 'Eigener Port' })
    .press('Enter');
  await expect(custom.nth(1)).not.toHaveAttribute('data-warn');
});
