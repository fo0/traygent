import { choose } from './support/dropdown';
import { expect, test, type Locator, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { AppSettings, Snapshot } from '../src/lib/types';

/*
 * Settings → Agenten → „Profile“ (#33, #37): adding applies at once, a row
 * opens its details in place, in the labelled demo and against the labelled
 * native-IPC mock (synthetic data only).
 */
const chip = (page: Page) =>
  page.locator('form.composer').getByRole('combobox', { name: /^Profil: / });

/** The profile block's success note (the dialog has other live regions). */
const noteIn = (dialog: Locator) => dialog.locator('.profiles [role="status"]');

async function openAgents(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  return page.getByRole('dialog', { name: 'Einstellungen' });
}

test('Settings → Agenten → „Profile“: an added profile signs in before „Speichern“', async ({
  page,
}) => {
  await page.goto('/');
  let dialog = await openAgents(page);
  const list = dialog.getByRole('list', { name: 'Profile' });
  await expect(list.getByRole('listitem')).toHaveCount(2);
  await expect(list.getByRole('listitem').first()).toContainText(
    'Standard demo@example.com',
  );
  // „Standard“ cannot be removed.
  await expect(
    list.getByRole('button', { name: 'Standard entfernen' }),
  ).toHaveCount(0);
  const save = dialog.getByRole('button', { name: 'Speichern', exact: true });
  await expect(save).toHaveAttribute('aria-disabled', 'true');

  const field = dialog.getByRole('textbox', { name: 'Neues Profil' });
  await field.fill('team');
  await field.press('Enter');
  await expect(dialog.getByText('Profil gibt es schon.')).toBeVisible();
  await field.fill('Kunde A');
  await expect(field).toHaveAttribute(
    'title',
    '%USERPROFILE%\\.claude-kunde-a',
  );
  await dialog.getByRole('button', { name: 'Profil hinzufügen' }).click();
  await expect(field).toHaveValue('');
  // Saved at once: it signs in now, and nothing waits for „Speichern“.
  const login = list.getByRole('button', { name: 'Kunde A anmelden' });
  await expect(login).not.toHaveAttribute('aria-disabled');
  await expect(save).toHaveAttribute('aria-disabled', 'true');
  await expect(
    list.getByRole('listitem').filter({ hasText: 'Kunde A' }),
  ).toContainText('Nicht angemeldet');
  await login.click();
  await expect(noteIn(dialog)).toHaveText('Demo: kein Terminal geöffnet.');
  await expect(list.getByText('kunde-a@example.com')).toBeVisible();
  // The note goes after 3 s; wait, so nothing shifts under the next clicks.
  await expect(noteIn(dialog)).toHaveCount(0, { timeout: 5000 });

  // The default and a removal stay staged.
  const standard = dialog.getByRole('combobox', { name: 'Standard-Profil' });
  await choose(standard, 'Team');
  await list.getByRole('button', { name: 'Team entfernen' }).click();
  await expect(standard).toHaveText('Standard');
  await choose(standard, 'Kunde A');

  await dialog.getByRole('button', { name: 'Jetzt synchronisieren' }).click();
  const note = noteIn(dialog);
  await expect(note).toHaveText('Profile synchronisiert (Demo).');
  await expect(note).toHaveCount(0, { timeout: 5000 });

  await save.click();
  await expect(dialog).toHaveCount(0);
  await expect(chip(page)).toHaveAccessibleName('Profil: Kunde A');

  dialog = await openAgents(page);
  await expect(
    dialog.getByRole('button', { name: 'Team anmelden' }),
  ).toHaveCount(0);
});

test('an added profile survives „Abbrechen“; the staged edits beside it do not', async ({
  page,
}) => {
  await page.goto('/');
  let dialog = await openAgents(page);
  await dialog.getByRole('button', { name: 'Team entfernen' }).click();
  const field = dialog.getByRole('textbox', { name: 'Neues Profil' });
  await field.fill('Kunde');
  await field.press('Enter');
  await expect(field).toHaveValue('');
  await dialog.getByRole('button', { name: 'Abbrechen' }).click();
  dialog = await openAgents(page);
  const list = dialog.getByRole('list', { name: 'Profile' });
  await expect(list.getByRole('button', { name: /Details$/ })).toHaveText([
    'Standard',
    'Team',
    'Kunde',
  ]);
});

test('a profile row opens its details in place, one at a time', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openAgents(page);
  const list = dialog.getByRole('list', { name: 'Profile' });
  const team = list.getByRole('button', { name: 'Team Details' });
  const standard = list.getByRole('button', { name: 'Standard Details' });
  await expect(team).toHaveAttribute('aria-expanded', 'false');
  // The row stays one line: no detail before it is opened.
  await expect(list.locator('.profile-detail')).toHaveCount(0);
  const before = await team.boundingBox();
  await team.click();
  await expect(team).toHaveAttribute('aria-expanded', 'true');
  const detail = list.locator('.profile-detail');
  await expect(detail).toHaveCount(1);
  await expect(detail).toContainText('Konto');
  await expect(detail).toContainText('team@example.com');
  await expect(detail).toContainText('Beispiel GmbH');
  await expect(detail).toContainText('Abgeglichen');
  await expect(
    detail.getByRole('textbox', { name: 'Ordner von Team' }),
  ).toHaveValue('C:\\Users\\demo\\.claude-team');
  // The toggle stays where it is.
  expect(await team.boundingBox()).toEqual(before);

  // A rename is staged and validated; id and folder stay.
  const name = detail.getByRole('textbox', { name: 'Name' });
  await name.fill('standard');
  await expect(detail.getByText('„standard“ ist reserviert.')).toBeVisible();
  await expect(name).toHaveAttribute('aria-invalid', 'true');
  await name.fill('Kunde B');
  await expect(
    list.getByRole('button', { name: 'Kunde B Details' }),
  ).toBeVisible();
  await expect(
    dialog.getByRole('button', { name: 'Speichern', exact: true }),
  ).not.toHaveAttribute('aria-disabled');

  // Keyboard: Enter on another toggle opens it and closes the first.
  await standard.focus();
  await page.keyboard.press('Enter');
  await expect(standard).toHaveAttribute('aria-expanded', 'true');
  await expect(
    list.getByRole('button', { name: 'Kunde B Details' }),
  ).toHaveAttribute('aria-expanded', 'false');
  await expect(list.locator('.profile-detail')).toHaveCount(1);
  // „Standard“ keeps its name; there is nothing to rename.
  await expect(
    list.locator('.profile-detail').getByRole('textbox', { name: 'Name' }),
  ).toHaveCount(0);
  await page.keyboard.press('Space');
  await expect(standard).toHaveAttribute('aria-expanded', 'false');
});

test('natively an add sends the saved settings plus the profile; a refusal keeps the name', async ({
  page,
}) => {
  await installTauriMock<Snapshot, { refuse: boolean }>(
    page,
    (snapshot, state) => {
      let current = snapshot;
      return async (cmd, args) => {
        if (cmd === 'snapshot' || cmd === 'refresh_detection') return current;
        if (cmd === 'save_settings') {
          if (state.refuse) {
            state.refuse = false;
            throw 'Höchstens 8 Profile für Claude Code möglich.';
          }
          const settings = args?.settings as AppSettings;
          // Natively the statuses are read anew: a new profile starts signed out.
          const added = settings.profiles.claude
            .filter((p) => !current.profiles.some((known) => known.id === p.id))
            .map((p) => ({
              ...p,
              agent: 'claude' as const,
              dir: '',
              signedIn: false,
            }));
          current = {
            ...current,
            settings,
            profiles: [...current.profiles, ...added],
          };
          return current;
        }
        if (cmd === 'autostart_status')
          return { status: 'disabled', registered: false };
        return null;
      };
    },
    { arg: nativeSnapshot(), state: { refuse: true } },
  );
  await page.goto('/');
  const dialog = await openAgents(page);
  await choose(
    dialog.getByRole('combobox', { name: 'Standard-Profil' }),
    'Team',
  );
  const field = dialog.getByRole('textbox', { name: 'Neues Profil' });
  await field.fill('Kunde');
  await field.press('Enter');
  await expect(
    dialog.getByText('Höchstens 8 Profile für Claude Code möglich.'),
  ).toBeVisible();
  await expect(field).toHaveValue('Kunde');
  await field.press('Enter');
  await expect(field).toHaveValue('');
  const calls = await readCalls(page, 'save_settings');
  expect(calls).toHaveLength(2);
  const sent = calls[1]?.args?.settings as AppSettings;
  expect(sent.profiles.claude.map((p) => p.id)).toEqual(['team', 'kunde']);
  // The staged default was not sent; it is still staged.
  expect(sent.agentProfile.claude).toBe('default');
  await expect(
    dialog.getByRole('combobox', { name: 'Standard-Profil' }),
  ).toHaveText('Team');
  await expect(
    dialog.getByRole('button', { name: 'Kunde anmelden' }),
  ).not.toHaveAttribute('aria-disabled');
});
