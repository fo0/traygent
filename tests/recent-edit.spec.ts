import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import { expect, test, type Page } from './support/test';
import type { LaunchRequest, Snapshot } from '../src/lib/types';

/*
 * „Bearbeiten …“ of a „Workspaces“ row (#3): the options and ports its ▶ sends,
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
  await dialog.getByLabel('Modell', { exact: true }).fill('opus');
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

test('a Docker row stores its Docker socket; a Lokal row has no switch', async ({
  page,
}) => {
  await page.goto('/');
  const local = await openEdit(page, 'Notizen');
  await expect(
    local.getByRole('checkbox', { name: 'Docker-Socket durchreichen' }),
  ).toHaveCount(0);
  await local.getByRole('button', { name: 'Abbrechen' }).click();
  // The row's start names the socket it passes (host-level access).
  const start = row(page, 'terraform-module').getByRole('button', {
    name: /starten$/,
  });
  await expect(start).toHaveAttribute(
    'title',
    /· Docker-Socket durchgereicht ·/,
  );
  const dialog = await openEdit(page, 'terraform-module');
  const socket = dialog.getByRole('checkbox', {
    name: 'Docker-Socket durchreichen',
  });
  await expect(socket).toBeChecked();
  const save = dialog.getByRole('button', { name: 'Speichern' });
  await expect(save).toHaveAttribute('aria-disabled', 'true');
  await dialog.getByText('Docker-Socket durchreichen').click();
  await expect(save).not.toHaveAttribute('aria-disabled', 'true');
  await save.click();
  await expect(dialog).toBeHidden();
  await expect(start).not.toHaveAttribute('title', /Docker-Socket/);
  const again = await openEdit(page, 'terraform-module');
  await expect(
    again.getByRole('checkbox', { name: 'Docker-Socket durchreichen' }),
  ).not.toBeChecked();
});

for (const name of ['traygent', 'kunden-portal'])
  test(`changes the Claude workspace profile for ${name}`, async ({ page }) => {
    await page.goto('/');
    const target = row(page, name);
    const mark = target.getByRole('img', { name: 'Profil Team' });
    await expect(mark).toHaveCount(0);
    const dialog = await openEdit(page, name);
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
    const again = await openEdit(page, name);
    await expect(again.getByRole('combobox', { name: 'Profil' })).toHaveText(
      'Team',
    );
    // A still-listed profile opens clean.
    await expect(
      again.getByRole('button', { name: 'Speichern' }),
    ).toHaveAttribute('aria-disabled', 'true');
    await again.getByRole('combobox', { name: 'Profil' }).click();
    await again.getByRole('option', { name: 'Standard' }).click();
    await again.getByRole('button', { name: 'Speichern' }).click();
    await expect(again).toBeHidden();
    await expect(mark).toHaveCount(0);
  });

const STALE: [string, (snapshot: Snapshot) => string[]][] = [
  [
    'a profile removed since',
    (snapshot) => {
      const notes = snapshot.recents.find((r) => r.folder.endsWith('Notizen'));
      notes!.profile = 'gone';
      return ['Standard', 'Team'];
    },
  ],
  [
    'no extra profile left',
    (snapshot) => {
      snapshot.settings.profiles.claude = [];
      snapshot.profiles = snapshot.profiles.filter(
        (p) => p.agent !== 'claude' || p.id === 'default',
      );
      return ['Standard'];
    },
  ],
];

for (const [name, stale] of STALE)
  test(`natively: ${name} shows and saves as „Standard“`, async ({ page }) => {
    const snapshot = nativeSnapshot();
    const options = stale(snapshot);
    const notes = snapshot.recents.find((r) => r.folder.endsWith('Notizen'))!;
    expect(notes.profile).toBeTruthy();
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
    const field = dialog.getByRole('combobox', { name: 'Profil' });
    await expect(field).toHaveText('Standard');
    await field.click();
    await expect(
      dialog.getByRole('listbox', { name: 'Profil' }).getByRole('option'),
    ).toHaveText(options);
    await field.press('Escape');
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
      keepAgentModel: false,
      dockerSocket: false,
    });
  });

test('„Modell & Denken“ „Agent“: fields off, a mark on the row, saved without a start', async ({
  page,
}) => {
  await page.goto('/');
  const target = row(page, 'Notizen');
  const dialog = await openEdit(page, 'Notizen');
  const model = dialog.getByRole('combobox', { name: 'Modell', exact: true });
  await model.fill('opus');
  await expect(model).toBeEnabled();
  await dialog.getByRole('radio', { name: 'Agent' }).check();
  // The values stay, only off.
  await expect(model).toBeDisabled();
  await expect(model).toHaveValue('opus');
  await expect(dialog.getByRole('combobox', { name: 'Denken' })).toBeDisabled();
  await dialog.getByRole('button', { name: 'Speichern' }).click();
  await expect(dialog).toBeHidden();
  const mark = target.getByRole('img', { name: 'Modell & Denken vom Agent' });
  await expect(mark).toBeVisible();
  await expect(target.getByRole('img', { name: /^Modell: / })).toHaveCount(0);
  await expect(
    target.getByRole('button', { name: /starten$/ }),
  ).toHaveAttribute('title', /Modell & Denken vom Agent/);
  // Back to „Traygent“: the stored model shows again.
  const again = await openEdit(page, 'Notizen');
  await expect(again.getByRole('radio', { name: 'Agent' })).toBeChecked();
  await again.getByRole('radio', { name: 'Traygent' }).check();
  await again.getByRole('button', { name: 'Speichern' }).click();
  await expect(mark).toHaveCount(0);
  await expect(target.getByRole('img', { name: /^Modell: / })).toBeVisible();
});

test('natively: a row set to „Agent“ starts with keepAgentModel', async ({
  page,
}) => {
  const snapshot = nativeSnapshot();
  const notes = snapshot.recents.find((r) => r.folder.endsWith('Notizen'))!;
  notes.keepAgentModel = true;
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd) => {
      if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
      if (cmd === 'launch_session')
        return {
          recent: snapshot.recents.find((r) => r.keepAgentModel)!,
          snapshot,
        };
      return null;
    },
    { arg: snapshot, state: {} },
  );
  await page.goto('/');
  await row(page, 'Notizen')
    .getByRole('button', { name: /starten$/ })
    .click();
  await expect(
    page.getByRole('status').filter({ hasText: 'gestartet.' }),
  ).toBeVisible();
  const [call] = await readCalls(page, 'launch_session');
  const request = call?.args?.request as LaunchRequest;
  expect(request.keepAgentModel).toBe(true);
  expect(request.options).toEqual(notes.options);
});

test('natively: a Docker row restarts with its stored socket and Claude profile', async ({
  page,
}) => {
  const snapshot = nativeSnapshot();
  const workspace = snapshot.recents.find(
    (r) => r.agent === 'claude' && r.env === 'docker',
  )!;
  workspace.dockerSocket = true;
  workspace.profile = 'team';
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd) => {
      if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
      if (cmd === 'launch_session')
        return {
          recent: snapshot.recents.find((r) => r.dockerSocket)!,
          snapshot,
        };
      return null;
    },
    { arg: snapshot, state: {} },
  );
  await page.goto('/');
  await row(page, 'kunden-portal')
    .getByRole('button', { name: /starten$/ })
    .click();
  const [call] = await readCalls(page, 'launch_session');
  const request = call?.args?.request as LaunchRequest;
  expect(request).toMatchObject({
    env: 'docker',
    profile: 'team',
    dockerSocket: true,
    folder: workspace.folder,
  });
});
