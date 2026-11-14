import { expect, test, type Locator, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { LaunchRequest, Snapshot } from '../src/lib/types';

/*
 * Starting from a GitHub repository (docs/adr/0005): the folder field's
 * classification and suggestions, and Settings → GitHub, in the labelled demo
 * (synthetic demo-user / demo-org repositories) and against the labelled
 * native-IPC mock.
 */
const composer = (page: Page) => page.locator('form.composer');
const field = (page: Page) =>
  composer(page).getByRole('combobox', { name: 'Ordner oder Repo' });
const suggestions = (page: Page) =>
  page.getByRole('listbox', { name: 'Repositories' });
const start = (page: Page) =>
  composer(page).getByRole('button', { name: 'Starten' });
const settingsDialog = (page: Page) =>
  page.getByRole('dialog', { name: 'Einstellungen' });
async function openGithubTab(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  const dialog = settingsDialog(page);
  await dialog.getByRole('tab', { name: 'GitHub' }).click();
  return dialog;
}
/** „Trennen“ with its inline confirmation, in an open GitHub tab. */
async function disconnectIn(dialog: Locator) {
  await dialog.getByRole('button', { name: 'Trennen' }).click();
  const confirm = dialog.getByRole('group', { name: 'demo-user trennen?' });
  await expect(
    confirm.getByRole('button', { name: 'Abbrechen' }),
  ).toBeFocused();
  await confirm.getByRole('button', { name: 'Trennen' }).click();
  await expect(
    dialog.getByRole('button', { name: 'Lokale Anmeldung prüfen' }),
  ).toBeFocused();
}
async function disconnect(page: Page) {
  const dialog = await openGithubTab(page);
  await disconnectIn(dialog);
  return dialog;
}

test('search text suggests repositories; ↓ and Enter pick one, which starts', async ({
  page,
}) => {
  await page.goto('/');
  const input = field(page);
  await input.fill('demo tray');
  const list = suggestions(page);
  await expect(list.getByRole('option')).toHaveText([
    'demo-user/traygent',
    'demo-user/tray-experiments',
  ]);
  await expect(input).toHaveAttribute('aria-expanded', 'true');
  await expect(input).toHaveAttribute(
    'aria-activedescendant',
    'folder-suggestions-0',
  );
  await expect(start(page)).toHaveAttribute('aria-disabled', 'true');
  await expect(start(page)).toHaveAttribute('title', 'Ordner oder Repo wählen');
  await input.press('ArrowDown');
  await expect(list.getByRole('option').nth(1)).toHaveAttribute(
    'aria-selected',
    'true',
  );
  // Enter on the open list picks instead of starting.
  await input.press('Enter');
  await expect(input).toHaveValue('demo-user/tray-experiments');
  await expect(list).toHaveCount(0);
  await expect(input).toHaveAttribute('aria-expanded', 'false');
  await expect(
    composer(page).getByRole('img', {
      name: 'GitHub-Repository demo-user/tray-experiments',
    }),
  ).toBeVisible();
  await expect(
    page.getByRole('status').filter({ hasText: 'gestartet' }),
  ).toHaveCount(0);

  await input.press('Enter');
  await expect(
    page
      .getByRole('status')
      .filter({ hasText: 'Claude Code gestartet (Demo).' }),
  ).toBeVisible();
  const first = page.locator('.recent-row').first();
  await expect(first.locator('.recent-name')).toHaveText('tray-experiments');
  await expect(first.locator('.recent-start')).toHaveAttribute(
    'title',
    / · C:\\Users\\demo\\Traygent\\demo-user\\tray-experiments · /,
  );
  await expect(
    first.getByRole('img', {
      name: 'GitHub-Repository demo-user/tray-experiments',
    }),
  ).toBeVisible();
});

test('a click picks, Escape closes then clears, private repositories carry a lock', async ({
  page,
}) => {
  await page.goto('/');
  const input = field(page);
  await input.fill('ORG');
  const options = suggestions(page).getByRole('option');
  await expect(options).toHaveCount(6);
  await expect(options.first()).toHaveAccessibleName(
    'demo-org/kunden-portal · privat',
  );
  await expect(options.first()).toHaveAttribute(
    'title',
    'Privat · Portal für Kunden (Demo)',
  );
  await input.press('Escape');
  await expect(suggestions(page)).toHaveCount(0);
  await expect(input).toHaveValue('ORG');
  await input.press('ArrowUp');
  await expect(options.last()).toHaveAttribute('aria-selected', 'true');
  await options.nth(2).click();
  await expect(input).toHaveValue('demo-org/design-system');
  await expect(input).toBeFocused();
  await expect(start(page)).not.toHaveAttribute('aria-disabled');
  // With no list open, Escape clears the field for the next target.
  await input.press('Escape');
  await expect(input).toHaveValue('');
  await expect(input).toBeFocused();
});

test('a pasted URL is a repository; a recent from one adopts as owner/name', async ({
  page,
}) => {
  await page.goto('/');
  const input = field(page);
  await input.fill('https://github.com/demo-org/docs/tree/main');
  await expect(
    composer(page).getByRole('img', {
      name: 'GitHub-Repository demo-org/docs',
    }),
  ).toBeVisible();
  await expect(suggestions(page)).toHaveCount(0);
  await expect(start(page)).not.toHaveAttribute('aria-disabled');
  await input.fill('C:\\Users\\demo\\x');
  await expect(composer(page).locator('.folder-kind')).toHaveCount(0);

  const row = page.locator('.recent-row').filter({
    has: page.locator('.recent-name', { hasText: /^repo-watch$/ }),
  });
  await row.getByRole('button', { name: 'Aktionen für repo-watch' }).click();
  await page.getByRole('menuitem', { name: 'In Composer übernehmen' }).click();
  await expect(input).toHaveValue('demo-user/repo-watch');
});

test('without an account search text offers „GitHub verbinden …“', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await disconnect(page);
  await dialog.getByRole('button', { name: 'Abbrechen', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  const input = field(page);
  await input.fill('tray');
  await expect(suggestions(page).getByRole('option')).toHaveText([
    'GitHub verbinden …',
  ]);
  await input.press('Enter');
  await expect(
    settingsDialog(page).getByRole('tab', { name: 'GitHub' }),
  ).toHaveAttribute('aria-selected', 'true');
  // A typed repository needs no account.
  await settingsDialog(page)
    .getByRole('button', { name: 'Abbrechen', exact: true })
    .click();
  await input.fill('demo-org/docs');
  await expect(start(page)).not.toHaveAttribute('aria-disabled');
});

test('GitHub tab: disconnect, connect by token, import the gh sign-in', async ({
  page,
}) => {
  await page.goto('/');
  const dialog = await openGithubTab(page);
  const account = dialog.locator('.github-account');
  await expect(account).toContainText('demo-user · Token');
  await expect(account).toContainText('12 Repos');
  await dialog.getByRole('button', { name: 'Aktualisieren' }).click();
  await expect(account).toContainText('12 Repos');
  // Cancelling the confirmation, by click or Escape, returns focus to „Trennen“.
  const trigger = account.getByRole('button', { name: 'Trennen' });
  const confirm = dialog.getByRole('group', { name: 'demo-user trennen?' });
  await trigger.click();
  await confirm.getByRole('button', { name: 'Abbrechen' }).click();
  await expect(trigger).toBeFocused();
  await trigger.press('Enter');
  await page.keyboard.press('Escape');
  await expect(confirm).toHaveCount(0);
  await expect(trigger).toBeFocused();
  await expect(dialog).toBeVisible();

  await disconnectIn(dialog);
  const token = dialog.getByLabel('Token', { exact: true });
  await expect(token).toHaveAttribute('type', 'password');
  const connect = dialog.getByRole('button', { name: 'Verbinden' });
  await expect(connect).toBeDisabled();
  await token.fill('falsch');
  await connect.click();
  await expect(dialog.getByRole('alert')).toHaveText(
    'GitHub hat das Token abgelehnt.',
  );
  await expect(token).toHaveAttribute('aria-invalid', 'true');
  await token.fill('ghp_demo1234');
  await token.press('Enter');
  await expect(account).toContainText('demo-user · Token');
  await expect(
    dialog.getByRole('button', { name: 'Aktualisieren' }),
  ).toBeFocused();

  await disconnectIn(dialog);
  await dialog.getByRole('button', { name: 'Lokale Anmeldung prüfen' }).click();
  const logins = dialog.getByRole('radiogroup', {
    name: 'Anmeldungen der GitHub CLI',
  });
  await expect(logins.getByRole('radio', { name: 'demo-user' })).toBeChecked();
  await expect(logins.getByRole('radio', { name: 'demo-ci' })).toBeDisabled();
  await expect(logins).toContainText('nicht übernehmbar');
  const importButton = dialog.getByRole('button', { name: 'Übernehmen' });
  await expect(importButton).toHaveAccessibleDescription(
    'Das Token von demo-user wird aus der GitHub CLI übernommen und im Windows-Tresor gespeichert.',
  );
  await importButton.click();
  await expect(account).toContainText('demo-user · gh CLI');
  // Account actions apply at once: nothing is staged.
  await expect(
    dialog.getByRole('button', { name: 'Speichern', exact: true }),
  ).toHaveAttribute('aria-disabled', 'true');
  await dialog.getByRole('button', { name: 'Abbrechen', exact: true }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Konto demo-user verbunden.' }),
  ).toBeVisible();
});

test('the clone root is staged and starts land in it', async ({ page }) => {
  await page.goto('/');
  const dialog = await openGithubTab(page);
  const root = dialog.getByRole('textbox', { name: 'Klon-Ordner' });
  await expect(root).toHaveAttribute(
    'placeholder',
    'C:\\Users\\demo\\Traygent',
  );
  await root.fill('Code');
  await expect(root).toHaveAccessibleDescription(
    'Ordner muss ein absoluter Windows-Pfad sein.',
  );
  const save = dialog.getByRole('button', { name: 'Speichern', exact: true });
  await expect(save).toHaveAttribute('aria-disabled', 'true');
  await root.fill('D:\\Code');
  await save.click();
  await expect(dialog).toHaveCount(0);
  await field(page).fill('demo-org/api-gateway');
  await start(page).click();
  const first = page.locator('.recent-row').first();
  await expect(first.locator('.recent-start')).toHaveAttribute(
    'title',
    / · D:\\Code\\demo-org\\api-gateway · /,
  );
});

test('▶ on a recent from a repository sends its folder with the repository', async ({
  page,
}) => {
  const snapshot = nativeSnapshot();
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd) => {
      if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
      if (cmd === 'launch_session')
        return { recent: snapshot.recents[0]!, snapshot };
      if (cmd === 'autostart_status')
        return { status: 'disabled', registered: false };
      return null;
    },
    { arg: snapshot, state: {} },
  );
  await page.goto('/');
  const recent = snapshot.recents.find((entry) => entry.repo)!;
  await page
    .getByRole('button', { name: 'Codex in repo-watch starten' })
    .click();
  // Rust reuses the folder while it exists, so a moved Klon-Ordner never
  // clones a second copy (#40).
  await expect
    .poll(async () =>
      (await readCalls(page, 'launch_session')).map((call) => {
        const { folder, repo } = call.args?.request as LaunchRequest;
        return { folder, repo };
      }),
    )
    .toEqual([{ folder: recent.folder, repo: recent.repo }]);
});

test('focusing the field reads a stale listing once, natively', async ({
  page,
}) => {
  const base = nativeSnapshot();
  const stale: Snapshot = {
    ...base,
    github: { ...base.github, listedAt: '2026-01-01T00:00:00Z' },
  };
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd) => {
      if (cmd === 'snapshot') return snapshot;
      if (cmd === 'github_refresh_repos')
        return {
          ...snapshot,
          revision: snapshot.revision + 1,
          github: { ...snapshot.github, loading: true },
        };
      if (cmd === 'autostart_status')
        return { status: 'disabled', registered: false };
      return null;
    },
    { arg: stale, state: {} },
  );
  await page.goto('/');
  await field(page).focus();
  await expect
    .poll(async () => (await readCalls(page, 'github_refresh_repos')).length)
    .toBe(1);
  await field(page).blur();
  await field(page).focus();
  await field(page).fill('tray');
  await expect(suggestions(page).getByRole('option').first()).toHaveText(
    'demo-user/traygent',
  );
  expect(await readCalls(page, 'github_refresh_repos')).toHaveLength(1);
});
