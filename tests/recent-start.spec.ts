import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls, readCommands } from './support/tauriMock';
import type { LaunchRequest, Snapshot } from '../src/lib/types';

/*
 * „Zuletzt“: a click anywhere on a row starts its session (owner request,
 * #17), once per press (src/lib/rowStart.ts); „⋯“ and its menu keep their
 * own actions, and a press that only closes a popup starts nothing
 * (src/lib/menu.ts → closedPopup). Runs against the labelled native-IPC
 * mock, synthetic snapshot only.
 */
const rows = (page: Page) => page.locator('.recent-row');
const row = (page: Page, name: string) =>
  rows(page).filter({
    has: page.locator('.recent-name', { hasText: new RegExp(`^${name}$`) }),
  });
/** Folders of every `launch_session` call so far. */
const launches = async (page: Page) =>
  (await readCalls(page, 'launch_session')).map(
    (call) => (call.args?.request as LaunchRequest).folder,
  );
const started = (page: Page) =>
  page.getByRole('status').filter({ hasText: 'gestartet.' });
/** A press on the row's folder name, far from „⋯“ and ▶. */
const onName = { position: { x: 60, y: 20 } };

/**
 * Answers a start with the unchanged snapshot, or rejects it while `fail`
 * is set: a failed start leaves no cooldown and ends `launching` at once,
 * so only the per-press guard can refuse the next press.
 */
async function serve(page: Page, fail = false) {
  const snapshot = nativeSnapshot();
  await installTauriMock<Snapshot, { fail: boolean }>(
    page,
    (snapshot, state) => async (cmd) => {
      if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
      if (cmd === 'launch_session') {
        if (state.fail) throw 'Ordner existiert nicht.';
        return { recent: snapshot.recents[0]!, snapshot };
      }
      return null;
    },
    { arg: snapshot, state: { fail } },
  );
  return snapshot;
}
const folderOf = (snapshot: Snapshot, name: string) =>
  snapshot.recents.find((recent) => recent.folder.endsWith(`\\${name}`))!
    .folder;

test('a click anywhere on a row starts it once; „⋯“ never starts', async ({
  page,
}) => {
  const snapshot = await serve(page);
  await page.goto('/');
  const target = row(page, 'terraform-module');
  // „⋯“ and an item of its menu keep their own action.
  await target
    .getByRole('button', { name: 'Aktionen für terraform-module' })
    .click();
  await page.getByRole('menuitem', { name: 'Ordner öffnen' }).click();
  await expect.poll(() => readCommands(page)).toContain('open_folder');
  expect(await launches(page)).toEqual([]);

  await target.click(onName);
  await expect(started(page)).toBeVisible();
  expect(await launches(page)).toEqual([
    folderOf(snapshot, 'terraform-module'),
  ]);
  // The row hovers like RepoWatch's rows, with the pointer of a button.
  await target.hover(onName);
  await expect(target).toHaveCSS(
    'background-color',
    await page.evaluate(() => {
      const probe = document.createElement('span');
      probe.style.color = 'var(--hover)';
      document.body.append(probe);
      const value = getComputedStyle(probe).color;
      probe.remove();
      return value;
    }),
  );
  await expect(target.locator('.recent-start')).toHaveCSS('cursor', 'pointer');
});

test('Tab reaches „⋯“ before the start at the far right', async ({ page }) => {
  await serve(page);
  await page.goto('/');
  const target = row(page, 'agent-tools');
  const more = target.getByRole('button', { name: 'Aktionen für agent-tools' });
  await more.focus();
  await page.keyboard.press('Tab');
  await expect(
    target.getByRole('button', { name: 'OpenCode in agent-tools starten' }),
  ).toBeFocused();
});

test('a press that closes the row menu starts nothing, on its row or another', async ({
  page,
}) => {
  const snapshot = await serve(page);
  await page.goto('/');
  const menu = page.getByRole('menu');
  const own = row(page, 'kunden-portal');
  await own.getByRole('button', { name: 'Aktionen für kunden-portal' }).click();
  await expect(menu).toBeVisible();
  await own.click(onName);
  await expect(menu).toHaveCount(0);
  expect(await launches(page)).toEqual([]);

  await rows(page)
    .first()
    .getByRole('button', { name: /^Aktionen für / })
    .click();
  await expect(menu).toBeVisible();
  const other = rows(page).nth(3);
  await other.click(onName);
  await expect(menu).toHaveCount(0);
  expect(await launches(page)).toEqual([]);
  // The next press is a start again.
  await other.click(onName);
  await expect(started(page)).toBeVisible();
  expect(await launches(page)).toEqual([folderOf(snapshot, 'agent-tools')]);
});

test('a press that closes the „Ports“ popover starts no row', async ({
  page,
}) => {
  await serve(page);
  await page.goto('/');
  const composer = page.locator('form.composer');
  await composer.getByRole('radio', { name: 'Docker' }).check();
  await composer.getByRole('button', { name: /^Ports\b/ }).click();
  const popover = page.getByRole('dialog', { name: 'Ports' });
  await expect(popover).toBeVisible();
  await rows(page).last().click(onName);
  await expect(popover).toHaveCount(0);
  expect(await launches(page)).toEqual([]);
});

test('a press that closes the folder suggestions starts no row', async ({
  page,
}) => {
  const snapshot = await serve(page);
  await page.goto('/');
  const field = page
    .locator('form.composer')
    .getByRole('combobox', { name: 'Ordner oder Repo' });
  await field.fill('demo tray');
  const list = page.getByRole('listbox', { name: 'Repositories' });
  await expect(list.getByRole('option')).not.toHaveCount(0);
  // The list closes on blur, not on the press (FolderField → pressClosedPopup).
  const last = rows(page).last();
  await last.click(onName);
  await expect(list).toHaveCount(0);
  await expect(field).toHaveAttribute('aria-expanded', 'false');
  expect(await launches(page)).toEqual([]);
  // The next press is a start again.
  await last.click(onName);
  await expect(started(page)).toBeVisible();
  expect(await launches(page)).toEqual([folderOf(snapshot, 'Notizen')]);
});

test('the rest of a double-click never starts again', async ({ page }) => {
  const snapshot = await serve(page, true);
  await page.goto('/');
  const folder = folderOf(snapshot, 'kunden-portal');
  await row(page, 'kunden-portal').dblclick(onName);
  await expect(page.getByRole('alert')).toHaveText('Ordner existiert nicht.');
  expect(await launches(page)).toEqual([folder]);
  // Neither a cooldown nor a running start refused the second click: a
  // separate click starts again.
  await row(page, 'kunden-portal').click(onName);
  await expect.poll(() => launches(page)).toEqual([folder, folder]);
});

test('a held Enter never starts again', async ({ page }) => {
  const snapshot = await serve(page, true);
  await page.goto('/');
  const folder = folderOf(snapshot, 'traygent');
  const start = row(page, 'traygent').getByRole('button', {
    name: 'Claude Code in traygent starten',
  });
  await start.focus();
  await page.keyboard.down('Enter');
  await expect(page.getByRole('alert')).toHaveText('Ordner existiert nicht.');
  // Auto-repeat while the key stays down.
  await page.keyboard.down('Enter');
  await page.keyboard.down('Enter');
  await page.keyboard.up('Enter');
  expect(await launches(page)).toEqual([folder]);
  // A new press starts again.
  await start.press('Enter');
  await expect.poll(() => launches(page)).toEqual([folder, folder]);
});
