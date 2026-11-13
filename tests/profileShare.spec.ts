import { expect, test, type Locator, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { AppSettings, Snapshot } from '../src/lib/types';

/*
 * The extra names Claude Code's profiles share (#35): Settings → Agenten →
 * „Profile“ → „Weitere Ordner“ / „Weitere Dateien“, staged like every
 * setting, in the labelled demo and against the labelled native-IPC mock
 * (synthetic data only).
 */
const CC = 'Claude Code';

async function openAgents(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  return page.getByRole('dialog', { name: 'Einstellungen' });
}
const dirs = (dialog: Locator) =>
  dialog.getByRole('textbox', { name: 'Weitere Ordner' });
const files = (dialog: Locator) =>
  dialog.getByRole('textbox', { name: 'Weitere Dateien' });
const save = (dialog: Locator) =>
  dialog.getByRole('button', { name: 'Speichern', exact: true });

test('the demo shares one folder and one file; an invalid name blocks „Speichern“', async ({
  page,
}) => {
  await page.goto('/');
  let dialog = await openAgents(page);
  await expect(dirs(dialog)).toHaveValue('.caveman-sessions');
  await expect(files(dialog)).toHaveValue('orca-state.json');
  await expect(dirs(dialog)).toHaveAttribute(
    'placeholder',
    '.caveman-sessions',
  );
  await expect(files(dialog)).toHaveAttribute('placeholder', 'orca-state.json');

  await dirs(dialog).fill('.caveman-sessions, skills');
  const error = `Name „skills“ für ${CC} wird bereits abgeglichen.`;
  await expect(dialog.getByText(error)).toBeVisible();
  await expect(dirs(dialog)).toHaveAttribute('aria-invalid', 'true');
  await expect(dirs(dialog)).toHaveAccessibleDescription(error);
  await expect(save(dialog)).toHaveAttribute('aria-disabled', 'true');
  // aria-disabled keeps it focusable; a click still saves nothing.
  await save(dialog).click({ force: true });
  await expect(dialog).toBeVisible();

  await files(dialog).fill('orca-state.json ORCA-STATE.json');
  await expect(
    dialog.getByText(`Name „ORCA-STATE.json“ für ${CC} ist doppelt vergeben.`),
  ).toBeVisible();
  await files(dialog).fill('orca-state.json, notes.md');
  await dirs(dialog).fill('.caveman-sessions notes');
  await expect(dialog.locator('.profile-share .field-error')).toHaveCount(0);
  await expect(save(dialog)).not.toHaveAttribute('aria-disabled');
  await save(dialog).click();
  await expect(dialog).toHaveCount(0);

  dialog = await openAgents(page);
  await expect(dirs(dialog)).toHaveValue('.caveman-sessions, notes');
  await expect(files(dialog)).toHaveValue('orca-state.json, notes.md');
});

test('the fields hide with the last extra profile and come back with a new one', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openAgents(page);
  await dirs(dialog).fill('notes');
  await dialog.getByRole('button', { name: 'Team entfernen' }).click();
  await expect(dirs(dialog)).toHaveCount(0);
  await expect(files(dialog)).toHaveCount(0);
  const field = dialog.getByRole('textbox', { name: 'Neues Profil' });
  await field.fill('Kunde');
  await field.press('Enter');
  // The staged names were kept.
  await expect(dirs(dialog)).toHaveValue('notes');
});

test('„Bereits abgeglichen“ lists the built-in folders and files, collapsed', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openAgents(page);
  const builtIn = dialog.locator('details.profile-builtin');
  const summary = builtIn.getByText('Bereits abgeglichen');
  await expect(summary).toBeVisible();
  await expect(builtIn).not.toHaveAttribute('open');
  await expect(
    builtIn.getByText('settings.json', { exact: false }),
  ).toBeHidden();
  await summary.click();
  await expect(builtIn).toHaveAttribute('open');
  const [dirsLine, filesLine] = (
    await builtIn.locator('dd').allTextContents()
  ).map((text) => text.replace(/\s+/g, ' ').trim());
  expect(dirsLine).toMatch(/^skills, agents, .*, session-env$/);
  expect(filesLine).toBe(
    'CLAUDE.md, keybindings.json, history.jsonl, settings.json',
  );
  await expect(builtIn.locator('dt')).toHaveText(['Ordner', 'Dateien']);
  // Keyboard: the summary toggles it back.
  await builtIn.locator('summary').press('Enter');
  await expect(builtIn).not.toHaveAttribute('open');
});

test('natively „Speichern“ sends the parsed names in `profileShare`', async ({
  page,
}) => {
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => {
      let current = snapshot;
      return async (cmd, args) => {
        if (cmd === 'snapshot' || cmd === 'refresh_detection') return current;
        if (cmd === 'save_settings') {
          current = { ...current, settings: args?.settings as AppSettings };
          return current;
        }
        if (cmd === 'autostart_status')
          return { status: 'disabled', registered: false };
        return null;
      };
    },
    { arg: nativeSnapshot(), state: {} },
  );
  await page.goto('/');
  const dialog = await openAgents(page);
  await dirs(dialog).fill(' .caveman-sessions,, notes ');
  await files(dialog).fill('');
  await save(dialog).click();
  await expect(dialog).toHaveCount(0);
  const calls = await readCalls(page, 'save_settings');
  const settings = calls[0]?.args?.settings as AppSettings;
  expect(settings.profileShare).toEqual({
    claude: { dirs: ['.caveman-sessions', 'notes'], files: [] },
    opencode: { dirs: [], files: [] },
    codex: { dirs: [], files: [] },
  });
});
