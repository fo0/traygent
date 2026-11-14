import { LAYOUT_FONTS, pinFont } from './support/fonts';
import { expect, test, type Locator, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { LaunchRequest, Snapshot } from '../src/lib/types';

/*
 * Agent profiles (#33): the composer's „Profil“ chip, the profile mark in
 * „Zuletzt“ and Settings → Agenten → „Profile“, in the labelled demo and
 * against the labelled native-IPC mock (synthetic data only).
 */
const composer = (page: Page) => page.locator('form.composer');
const chip = (page: Page) =>
  composer(page).getByRole('combobox', { name: /^Profil: / });
const row = (page: Page, name: string) =>
  page.locator('.recent-row').filter({
    has: page.locator('.recent-name', { hasText: new RegExp(`^${name}$`) }),
  });

/** The profile block's success note (the dialog has other live regions). */
const noteIn = (dialog: Locator) => dialog.locator('.profiles [role="status"]');

async function openAgents(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  return page.getByRole('dialog', { name: 'Einstellungen' });
}

test('the „Profil“ chip: Claude Code in Lokal only, for the next start', async ({
  page,
}) => {
  await page.goto('/');
  await expect(chip(page)).toHaveAccessibleName('Profil: Standard');
  await expect(chip(page)).toHaveAttribute('title', /^Profil: Standard/);
  await composer(page).getByRole('radio', { name: 'Docker' }).check();
  await expect(chip(page)).toHaveCount(0);
  await composer(page).getByRole('radio', { name: 'Lokal' }).check();
  await composer(page).getByRole('radio', { name: 'Codex' }).check();
  await expect(chip(page)).toHaveCount(0);
  await composer(page).getByRole('radio', { name: 'Claude Code' }).check();

  await chip(page).click();
  const list = page.getByRole('listbox', { name: 'Profil' });
  await expect(list.getByRole('option')).toHaveText(['Standard', 'Team']);
  await list.getByRole('option', { name: 'Team' }).click();
  await expect(chip(page)).toHaveAccessibleName('Profil: Team');
  await expect(chip(page)).toHaveText('T');

  await composer(page)
    .getByRole('combobox', { name: 'Ordner oder Repo' })
    .fill('C:\\Users\\demo\\Projekte\\neu');
  await composer(page).getByRole('button', { name: 'Starten' }).click();
  await expect(
    row(page, 'neu').getByRole('img', { name: 'Profil Team' }),
  ).toBeVisible();
  // The next start begins from the saved default again.
  await expect(chip(page)).toHaveAccessibleName('Profil: Standard');
});

for (const font of LAYOUT_FONTS)
  test(`the chip keeps one row beside „Starten“ at 480 px in ${font}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 480, height: 760 });
    await page.goto('/');
    await pinFont(page, font);
    await expect(chip(page)).toBeVisible();
    const tops = await composer(page).evaluate((node) => {
      const parts = [
        ...node.querySelectorAll('.option-chips .chip, .composer-start'),
      ];
      return parts.map((part) =>
        Math.round(part.getBoundingClientRect().bottom),
      );
    });
    expect(new Set(tops).size, tops.join(', ')).toBe(1);
  });

test('„Zuletzt“ shows a profile on the agent tile, taking no width from the row', async ({
  page,
}) => {
  await page.setViewportSize({ width: 480, height: 760 });
  await page.goto('/');
  const notes = row(page, 'Notizen');
  const mark = notes.getByRole('img', { name: 'Profil Team' });
  await expect(mark).toHaveText('T');
  await expect(page.locator('.recent-row .profile-mark')).toHaveCount(1);
  // On the tile's corner: inside the row, overlapping the 22 px tile.
  const tile = await notes.locator('.agent-badge').boundingBox();
  const box = await mark.boundingBox();
  expect(box!.x).toBeGreaterThan(tile!.x);
  expect(box!.x).toBeLessThan(tile!.x + tile!.width);
  expect(box!.y).toBeLessThan(tile!.y + tile!.height);
  // The tile keeps its width; the start's tooltip names the profile.
  expect(
    await notes.locator('.agent-tile').evaluate((n) => n.clientWidth),
  ).toBe(22);
  await expect(notes.getByRole('button', { name: /starten$/ })).toHaveAttribute(
    'title',
    /· Lokal · Profil: Team ·/,
  );
});

test('natively: ▶ restarts with the stored profile, a warning stays, sign-in names the profile', async ({
  page,
}) => {
  const snapshot = nativeSnapshot();
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd) => {
      if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
      if (cmd === 'launch_session')
        return {
          recent: snapshot.recents[0]!,
          snapshot,
          warning: 'Profil „Team“ ist nicht abgeglichen.',
        };
      if (cmd === 'sync_profiles') return snapshot;
      return null;
    },
    { arg: snapshot, state: {} },
  );
  await page.goto('/');
  await row(page, 'Notizen')
    .getByRole('button', { name: /starten$/ })
    .click();
  const notice = page.getByRole('status').filter({ hasText: 'gestartet.' });
  await expect(notice).toHaveText(
    /Claude Code gestartet\. Profil „Team“ ist nicht abgeglichen\./,
  );
  await page.waitForTimeout(3500);
  await expect(notice).toHaveCount(1);
  const launch = await readCalls(page, 'launch_session');
  expect((launch[0]?.args?.request as LaunchRequest).profile).toBe('team');

  const dialog = await openAgents(page);
  await dialog.getByRole('button', { name: 'Team anmelden' }).click();
  await expect(noteIn(dialog)).toHaveText('Anmeldung geöffnet.');
  expect((await readCalls(page, 'profile_login'))[0]?.args).toEqual({
    agent: 'claude',
    profile: 'team',
  });
  await dialog.getByRole('button', { name: 'Jetzt synchronisieren' }).click();
  await expect(noteIn(dialog)).toHaveText('Profile synchronisiert.');
  expect(await readCalls(page, 'sync_profiles')).toHaveLength(1);
});
