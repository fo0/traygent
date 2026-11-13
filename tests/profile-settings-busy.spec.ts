import { choose } from './support/dropdown';
import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { AppSettings, Snapshot } from '../src/lib/types';

/*
 * Settings → Agenten → „Profile“ while an add runs (#37), against the
 * labelled native-IPC mock (synthetic data only): the dialog cannot close on
 * a half-applied add, and a dialog opened afterwards starts from the answer.
 */
async function openAgents(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  return page.getByRole('dialog', { name: 'Einstellungen' });
}

test('the dialog stays open during an add; the next one starts from its answer', async ({
  page,
}) => {
  const mock = await installTauriMock<Snapshot, { hold: boolean }>(
    page,
    (snapshot, state) => {
      let current = snapshot;
      return async (cmd, args) => {
        if (cmd === 'snapshot' || cmd === 'refresh_detection') return current;
        if (cmd === 'save_settings') {
          while (state.hold) await new Promise((done) => setTimeout(done, 50));
          const settings = args?.settings as AppSettings;
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
    { arg: nativeSnapshot(), state: { hold: false } },
  );
  await page.goto('/');
  let dialog = await openAgents(page);
  await mock.configure({ hold: true });
  const field = dialog.getByRole('textbox', { name: 'Neues Profil' });
  await field.fill('Kunde');
  await field.press('Enter');
  const busy = dialog.getByRole('button', { name: 'Profil wird hinzugefügt' });
  await expect(busy).toHaveAttribute('aria-disabled', 'true');
  await expect(busy).toHaveAttribute('title', 'Wird hinzugefügt …');
  await expect(field).toHaveAttribute('aria-busy', 'true');
  const save = dialog.getByRole('button', { name: 'Speichern', exact: true });
  await expect(save).toHaveAttribute('title', 'Profil wird hinzugefügt …');
  await expect(
    dialog.getByRole('button', { name: 'Abbrechen' }),
  ).toBeDisabled();
  await page.keyboard.press('Escape');
  const close = dialog.getByRole('button', { name: 'Dialog schließen' });
  await expect(close).toHaveAttribute('aria-disabled', 'true');
  await close.click({ force: true });
  await expect(dialog).toBeVisible();

  await mock.configure({ hold: false });
  await expect(field).toHaveValue('');
  await expect(
    dialog.getByRole('button', { name: 'Profil hinzufügen' }),
  ).not.toHaveAttribute('aria-disabled');
  await expect(save).toHaveAttribute('title', 'Keine Änderungen');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);

  // Reopened, the dialog starts from the saved answer: „Speichern“ keeps it.
  dialog = await openAgents(page);
  await choose(
    dialog.getByRole('combobox', { name: 'Standard-Profil' }),
    'Kunde',
  );
  await dialog.getByRole('button', { name: 'Speichern', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  const calls = await readCalls(page, 'save_settings');
  expect(calls).toHaveLength(2);
  const sent = calls[1]?.args?.settings as AppSettings;
  expect(sent.profiles.claude.map((p) => p.id)).toEqual(['team', 'kunde']);
  expect(sent.agentProfile.claude).toBe('kunde');
});
