import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { AppSettings, Snapshot } from '../src/lib/types';

/*
 * Signing a profile in from Settings → Agenten → „Profile“ (#41): only a
 * signed-out row offers „Anmelden“, a signed-in one „Neu anmelden“ in its
 * details, rows with the same account name their organization, a hidden
 * sign-in can be cancelled and a failure stays inline. In the labelled demo
 * and against the labelled native-IPC mock (synthetic data only).
 */
async function openAgents(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  return page.getByRole('dialog', { name: 'Einstellungen' });
}

const rowOf = (page: Page, name: string) =>
  page
    .getByRole('list', { name: 'Profile' })
    .getByRole('listitem')
    .filter({ has: page.getByRole('button', { name: `${name} Details` }) });

test('only a signed-out row offers „Anmelden“; the demo sign-in flips it in place', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openAgents(page);
  const standard = rowOf(page, 'Standard');
  const team = rowOf(page, 'Team');
  await expect(standard.locator('.profile-row')).toContainText(
    'demo@example.com',
  );
  await expect(standard.getByRole('button', { name: /anmelden$/ })).toHaveCount(
    0,
  );
  await expect(team.locator('.profile-row')).toContainText('Nicht angemeldet');
  // „Neu anmelden“ sits in a signed-in row's details.
  await standard.getByRole('button', { name: 'Standard Details' }).click();
  await expect(
    standard.getByRole('button', { name: 'Standard neu anmelden' }),
  ).toBeVisible();
  await expect(
    team.getByRole('button', { name: 'Team neu anmelden' }),
  ).toHaveCount(0);

  await team.getByRole('button', { name: 'Team anmelden' }).click();
  const waiting = team.getByRole('button', { name: 'Anmeldung läuft' });
  await expect(waiting).toHaveAttribute('title', 'Abbrechen');
  await expect(dialog.locator('.profiles .install-note')).toHaveText(
    'Angemeldet (Demo).',
  );
  await expect(waiting).toHaveCount(0);
  await expect(team.getByRole('button', { name: 'Team anmelden' })).toHaveCount(
    0,
  );
  // One account in two organizations: each row shows its organization, the
  // tooltip both values.
  const account = team.locator('.profile-account');
  await expect(account).toHaveText('Beispiel Team');
  await expect(account).toHaveAttribute(
    'title',
    'demo@example.com · Beispiel Team',
  );
  await expect(standard.locator('.profile-account')).toHaveText(
    'Beispiel GmbH',
  );
});

test('a running sign-in is cancelled from its row and leaves no error', async ({
  page,
}) => {
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => {
      let cancel: (() => void) | undefined;
      return async (cmd) => {
        if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
        if (cmd === 'profile_login')
          return new Promise((_, reject) => {
            cancel = () => reject('Anmeldung abgebrochen.');
          });
        if (cmd === 'cancel_profile_login') cancel?.();
        if (cmd === 'autostart_status')
          return { status: 'disabled', registered: false };
        return null;
      };
    },
    { arg: nativeSnapshot(), state: {} },
  );
  await page.goto('/');
  const dialog = await openAgents(page);
  const team = rowOf(page, 'Team');
  await team.getByRole('button', { name: 'Team anmelden' }).click();
  await team.getByRole('button', { name: 'Anmeldung läuft' }).click();
  await expect(
    team.getByRole('button', { name: 'Team anmelden' }),
  ).toBeVisible();
  await expect(dialog.getByRole('alert')).toHaveCount(0);
  expect((await readCalls(page, 'cancel_profile_login'))[0]?.args).toEqual({
    agent: 'claude',
    profile: 'team',
  });
});

test('a failed hidden sign-in stays inline with the „Debug“ hint as tooltip', async ({
  page,
}) => {
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd) => {
      if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
      if (cmd === 'profile_login') throw 'Anmeldung fehlgeschlagen.';
      if (cmd === 'sync_profiles') return snapshot;
      if (cmd === 'autostart_status')
        return { status: 'disabled', registered: false };
      return null;
    },
    { arg: nativeSnapshot(), state: {} },
  );
  await page.goto('/');
  const dialog = await openAgents(page);
  await dialog.getByRole('button', { name: 'Team anmelden' }).click();
  const alert = dialog.getByRole('alert');
  await expect(alert).toHaveText('Anmeldung fehlgeschlagen.');
  await expect(alert).toHaveAttribute('title', 'Mit Debug erneut versuchen');
  // It stays until the next action.
  await page.waitForTimeout(3500);
  await expect(alert).toBeVisible();
  await dialog.getByRole('button', { name: 'Jetzt synchronisieren' }).click();
  await expect(alert).toHaveCount(0);
});

test('„Debug“ is a staged switch under App; with it the sign-in opens a terminal', async ({
  page,
}) => {
  await page.goto('/');
  let dialog = await openAgents(page);
  await dialog.getByRole('tab', { name: /^App/ }).click();
  const debug = dialog.getByRole('checkbox', { name: 'Debug' });
  await expect(debug).not.toBeChecked();
  await expect(dialog.locator('.debug-setting .switch-row')).toHaveAttribute(
    'title',
    'Anmeldung im sichtbaren Terminal, Fehler mit Ausgabe',
  );
  await dialog.getByText('Debug', { exact: true }).click();
  await expect(debug).toBeChecked();
  await dialog.getByRole('button', { name: 'Speichern', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  dialog = await openAgents(page);
  const team = rowOf(page, 'Team');
  await team.getByRole('button', { name: 'Team anmelden' }).click();
  await expect(dialog.locator('.profiles .install-note')).toHaveText(
    'Demo: kein Terminal geöffnet.',
  );
  // No waiting state: the status follows as an event, without a reload.
  await expect(team.locator('.profile-account')).toHaveText('Beispiel Team');
});

test('natively the sign-in answer is shown at once', async ({ page }) => {
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd) => {
      if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
      if (cmd === 'profile_login')
        return {
          ...snapshot,
          revision: snapshot.revision + 1,
          profiles: snapshot.profiles.map((p) =>
            p.id === 'team'
              ? { ...p, signedIn: true, account: 'eva@example.com' }
              : p,
          ),
        };
      if (cmd === 'autostart_status')
        return { status: 'disabled', registered: false };
      return null;
    },
    { arg: nativeSnapshot(), state: {} },
  );
  await page.goto('/');
  const dialog = await openAgents(page);
  await dialog.getByRole('button', { name: 'Team anmelden' }).click();
  await expect(dialog.locator('.profiles .install-note')).toHaveText(
    'Angemeldet.',
  );
  // Another account: no organization needed, each row shows its account.
  await expect(rowOf(page, 'Team').locator('.profile-account')).toHaveText(
    'eva@example.com',
  );
  await expect(rowOf(page, 'Standard').locator('.profile-account')).toHaveText(
    'demo@example.com',
  );
  const settings = (await readCalls(page, 'save_settings'))[0]?.args
    ?.settings as AppSettings | undefined;
  expect(settings).toBeUndefined();
});
