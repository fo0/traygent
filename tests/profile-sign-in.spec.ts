import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { Snapshot } from '../src/lib/types';

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
  // The end state only: no assertion races the demo's own timer.
  await expect(team.locator('.profile-account')).toHaveText('Beispiel Team');
  await expect(dialog.locator('.profiles .install-note')).toHaveText(
    'Angemeldet (Demo).',
  );
  await expect(team.getByRole('button', { name: /anmelden$/ })).toHaveCount(0);
  // The focus moved from the gone button to the row's toggle, never <body>.
  await expect(
    team.getByRole('button', { name: 'Team Details' }),
  ).toBeFocused();
  // One account in two organizations: each row shows its organization, the
  // tooltip both values.
  await expect(team.locator('.profile-account')).toHaveAttribute(
    'title',
    'demo@example.com · Beispiel Team',
  );
  await expect(standard.locator('.profile-account')).toHaveText(
    'Beispiel GmbH',
  );
});

/** A native mock whose `profile_login` waits until the test releases it. */
async function heldSignIn(
  page: Page,
  found = true,
  arg: Snapshot = nativeSnapshot(),
) {
  return installTauriMock<
    Snapshot,
    { release: '' | 'signedIn' | 'failed'; found: boolean }
  >(
    page,
    (snapshot, state) => {
      let cancel: (() => void) | undefined;
      return async (cmd) => {
        if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
        if (cmd === 'sync_profiles') return snapshot;
        if (cmd === 'profile_login')
          return new Promise((resolve, reject) => {
            cancel = () => reject('Anmeldung abgebrochen.');
            const wait = () => {
              if (state.release === 'failed')
                return reject('Anmeldung fehlgeschlagen.');
              if (state.release === 'signedIn')
                return resolve({
                  ...snapshot,
                  revision: snapshot.revision + 1,
                  profiles: snapshot.profiles.map((p) =>
                    p.id === 'team'
                      ? { ...p, signedIn: true, account: 'eva@example.com' }
                      : p,
                  ),
                });
              setTimeout(wait, 20);
            };
            wait();
          });
        if (cmd === 'cancel_profile_login') {
          if (state.found) cancel?.();
          return state.found;
        }
        if (cmd === 'autostart_status')
          return { status: 'disabled', registered: false };
        return null;
      };
    },
    { arg, state: { release: '', found } },
  );
}

test('a pending sign-in turns „Anmelden“ into its cancel on the same button', async ({
  page,
}) => {
  await heldSignIn(page);
  await page.goto('/');
  const dialog = await openAgents(page);
  const team = rowOf(page, 'Team');
  const button = team.locator('.profile-row .profile-login');
  await expect(button).toHaveAccessibleName('Team anmelden');
  await button.click();
  await expect(button).toHaveAccessibleName('Team: Anmeldung abbrechen');
  await expect(button).toHaveAttribute('aria-busy', 'true');
  await expect(button).toHaveAttribute('title', 'Anmeldung läuft · Abbrechen');
  await expect(button).toBeFocused();
  await button.click();
  await expect(button).toHaveAccessibleName('Team anmelden');
  await expect(button).toBeFocused();
  await expect(dialog.getByRole('alert')).toHaveCount(0);
  expect((await readCalls(page, 'cancel_profile_login'))[0]?.args).toEqual({
    agent: 'claude',
    profile: 'team',
  });
});

test('natively the answer flips the row and the focus moves to its toggle', async ({
  page,
}) => {
  const mock = await heldSignIn(page);
  await page.clock.install();
  await page.goto('/');
  const dialog = await openAgents(page);
  const team = rowOf(page, 'Team');
  await team.getByRole('button', { name: 'Team anmelden' }).click();
  await expect(
    team.getByRole('button', { name: 'Team: Anmeldung abbrechen' }),
  ).toBeVisible();
  await mock.configure({ release: 'signedIn' });
  const note = dialog.locator('.profiles .install-note');
  await expect(note).toHaveText('Angemeldet.');
  // Another account: no organization needed, each row shows its account.
  await expect(team.locator('.profile-account')).toHaveText('eva@example.com');
  await expect(rowOf(page, 'Standard').locator('.profile-account')).toHaveText(
    'demo@example.com',
  );
  await expect(
    team.getByRole('button', { name: 'Team Details' }),
  ).toBeFocused();
  await page.clock.fastForward(3100);
  await expect(note).toHaveCount(0);
});

test('a failed hidden sign-in stays inline, named, with the „Debug“ hint as tooltip', async ({
  page,
}) => {
  const mock = await heldSignIn(page, false);
  await page.clock.install();
  await page.goto('/');
  const dialog = await openAgents(page);
  await dialog.getByRole('button', { name: 'Team anmelden' }).click();
  // Rust had nothing to cancel yet: the later failure is still reported.
  await dialog
    .getByRole('button', { name: 'Team: Anmeldung abbrechen' })
    .click();
  await mock.configure({ release: 'failed' });
  const alert = dialog.getByRole('alert');
  await expect(alert).toHaveText('Team: Anmeldung fehlgeschlagen.');
  await expect(alert).toHaveAttribute('title', 'Mit Debug erneut versuchen');
  // It is no success note: it stays.
  await page.clock.fastForward(10_000);
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
  // The status follows as an event, without a reload.
  await expect(team.locator('.profile-account')).toHaveText('Beispiel Team');
});

test('the rest of a double-click or a held Enter never cancels the sign-in it started', async ({
  page,
}) => {
  await heldSignIn(page);
  await page.goto('/');
  await openAgents(page);
  const button = rowOf(page, 'Team').locator('.profile-row .profile-login');
  await button.dblclick();
  await expect(button).toHaveAccessibleName('Team: Anmeldung abbrechen');
  expect(await readCalls(page, 'cancel_profile_login')).toHaveLength(0);
  expect(await readCalls(page, 'profile_login')).toHaveLength(1);
  // A held Enter on the cancel: only its first press acts.
  const standard = rowOf(page, 'Standard');
  await standard.getByRole('button', { name: 'Standard Details' }).focus();
  await button.focus();
  await page.keyboard.down('Enter');
  await page.keyboard.down('Enter');
  await page.keyboard.up('Enter');
  await expect(button).toHaveAccessibleName('Team anmelden');
  expect(await readCalls(page, 'profile_login')).toHaveLength(1);
});

test('with „Debug“ the button only waits for the terminal: no cancel', async ({
  page,
}) => {
  const snapshot = nativeSnapshot();
  snapshot.settings.debug = true;
  const mock = await heldSignIn(page, true, snapshot);
  await page.goto('/');
  const dialog = await openAgents(page);
  const button = rowOf(page, 'Team').locator('.profile-row .profile-login');
  await button.click();
  await expect(button).toHaveAttribute('aria-busy', 'true');
  await expect(button).toHaveAttribute('aria-disabled', 'true');
  await expect(button).toHaveAccessibleName('Team anmelden');
  await expect(button).toHaveAttribute('title', 'Im Browser anmelden');
  // Off via aria-disabled: a click reaches it and does nothing.
  await button.click({ force: true });
  expect(await readCalls(page, 'cancel_profile_login')).toHaveLength(0);
  expect(await readCalls(page, 'profile_login')).toHaveLength(1);
  await mock.configure({ release: 'signedIn' });
  await expect(dialog.locator('.profiles .install-note')).toHaveText(
    'Anmeldung geöffnet.',
  );
});
