import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { AppSettings, Snapshot } from '../src/lib/types';

/** Settings → „Updates“, where the „Agenten“ group sits below the app update. */
async function openUpdates(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await dialog.getByRole('tab', { name: 'Updates', exact: true }).click();
  return dialog;
}
const autoUpdate = (dialog: ReturnType<Page['getByRole']>) =>
  dialog.getByRole('checkbox', { name: 'Automatisch aktualisieren' });
const saveButton = (dialog: ReturnType<Page['getByRole']>) =>
  dialog.getByRole('button', { name: 'Speichern', exact: true });

test('„Automatisch aktualisieren“ is staged: „Abbrechen“ drops it, „Speichern“ keeps it', async ({
  page,
}) => {
  await page.goto('/');
  let dialog = await openUpdates(page);
  const group = dialog.getByRole('region', { name: 'Agenten' });
  await expect(group).toContainText('Alle 4 Stunden, nur ohne offene Session.');
  await expect(autoUpdate(dialog)).not.toBeChecked();
  await expect(saveButton(dialog)).toHaveAttribute('aria-disabled', 'true');
  await dialog.getByText('Automatisch aktualisieren').click();
  await expect(autoUpdate(dialog)).toBeChecked();
  await expect(saveButton(dialog)).not.toHaveAttribute('aria-disabled');
  await dialog.getByRole('button', { name: 'Abbrechen', exact: true }).click();
  await expect(dialog).toHaveCount(0);

  dialog = await openUpdates(page);
  await expect(autoUpdate(dialog)).not.toBeChecked();
  await autoUpdate(dialog).check();
  await saveButton(dialog).click();
  await expect(dialog).toHaveCount(0);
  dialog = await openUpdates(page);
  await expect(autoUpdate(dialog)).toBeChecked();
});

test('„Jetzt aktualisieren“ takes one agent after the other and settles', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openUpdates(page);
  const list = dialog.getByRole('list', { name: 'Agenten-Updates' });
  await expect(list).toHaveCount(0);
  // Every row set seen with an agent `running`: the run's steps are too short
  // to poll for, so the page records them.
  await page.evaluate(() => {
    const seen: string[][] = [];
    (window as unknown as { runningSeen: string[][] }).runningSeen = seen;
    new MutationObserver(() => {
      const running = [...document.querySelectorAll('.agent-update-list li')]
        .filter((row) => row.textContent?.includes('wird aktualisiert'))
        .map(
          (row) => row.querySelector('.agent-update-name')?.textContent ?? '',
        );
      if (running.length && running.join() !== seen.at(-1)?.join())
        seen.push(running);
    }).observe(document.body, {
      childList: true,
      subtree: true,
      characterData: true,
    });
  });
  const button = dialog.getByRole('button', { name: 'Jetzt aktualisieren' });
  await button.click();
  await expect(button).toHaveAttribute('aria-disabled', 'true');
  const rows = list.getByRole('listitem');

  // Only ready local CLIs: OpenCode is not installed locally in the demo.
  await expect(rows).toHaveCount(2);
  await expect(button).not.toHaveAttribute('aria-disabled');
  await expect(rows.nth(0)).toContainText('aktualisiert auf 2.1.318');
  const updated = rows.nth(0).locator('.agent-update-state');
  await expect(updated).toHaveAttribute('title', '2.1.317 → 2.1.318');
  // The tooltip's detail is read with the row, never shown.
  await expect(rows.nth(0)).toHaveAccessibleName(
    'Claude Code aktualisiert auf 2.1.318 (vorher 2.1.317) gerade eben',
  );
  await expect(updated).toHaveText('aktualisiert auf 2.1.318 (vorher 2.1.317)');
  await expect(updated.getByText('(vorher 2.1.317)')).toHaveClass('sr-only');
  await expect(rows.nth(1)).toContainText('Codex');
  await expect(rows.nth(1)).toContainText('aktuell · 0.166.0');
  await expect(rows.nth(1)).toContainText('gerade eben');
  // One agent at a time, never all at once.
  expect(
    await page.evaluate(
      () => (window as unknown as { runningSeen: string[][] }).runningSeen,
    ),
  ).toEqual([['Claude Code'], ['Codex']]);
  // The rows are announced politely as they change.
  await expect(
    dialog.getByRole('region', { name: 'Agenten' }).getByRole('status'),
  ).toHaveAttribute('aria-relevant', 'additions');
  // An immediate action stages nothing.
  await expect(saveButton(dialog)).toHaveAttribute('aria-disabled', 'true');
});

test('natively the button stays off while a run is in progress', async ({
  page,
}) => {
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd) => {
      if (cmd === 'snapshot') return snapshot;
      if (cmd === 'autostart_status')
        return { status: 'disabled', registered: false };
      return null;
    },
    // No agent `running` yet: the flag alone marks the run.
    { arg: nativeSnapshot({ agentUpdatesRunning: true }), state: {} },
  );
  await page.goto('/');
  const dialog = await openUpdates(page);
  const button = dialog.getByRole('button', { name: 'Jetzt aktualisieren' });
  await expect(button).toHaveAttribute('aria-disabled', 'true');
  await expect(dialog.getByRole('region', { name: 'Agenten' })).toHaveAttribute(
    'aria-busy',
    'true',
  );
  // `force`: an aria-disabled button stays clickable, it just does nothing.
  await button.click({ force: true });
  expect(await readCalls(page, 'update_agents')).toEqual([]);
});

const REFUSALS = [
  'Agenten werden bereits aktualisiert.',
  'Die Agenten werden noch gesucht. Bitte gleich noch einmal versuchen.',
  'Kein lokaler Agent installiert.',
];

test('natively the setting saves with the settings and a refused run shows its error', async ({
  page,
}) => {
  await installTauriMock<Snapshot, { refusals: string[] }>(
    page,
    (snapshot, state) => {
      let current = snapshot;
      let refused = 0;
      return async (cmd, args) => {
        if (cmd === 'snapshot') return current;
        if (cmd === 'save_settings') {
          current = { ...current, settings: args?.settings as AppSettings };
          return current;
        }
        if (cmd === 'update_agents') throw state.refusals[refused++];
        if (cmd === 'autostart_status')
          return { status: 'disabled', registered: false };
        return null;
      };
    },
    { arg: nativeSnapshot(), state: { refusals: REFUSALS } },
  );
  await page.goto('/');
  const dialog = await openUpdates(page);
  const alert = dialog
    .getByRole('region', { name: 'Agenten' })
    .getByRole('alert');
  for (const refusal of REFUSALS) {
    await dialog.getByRole('button', { name: 'Jetzt aktualisieren' }).click();
    await expect(alert).toHaveText(refusal);
  }
  expect(
    (await readCalls(page, 'update_agents')).map((call) => call.cmd),
  ).toEqual(['update_agents', 'update_agents', 'update_agents']);

  await autoUpdate(dialog).check();
  await saveButton(dialog).click();
  await expect(dialog).toHaveCount(0);
  const expected = nativeSnapshot().settings;
  expected.agentUpdates = true;
  expect(
    (await readCalls(page, 'save_settings')).map((call) => call.args),
  ).toEqual([{ settings: expected }]);
});
