import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { choose } from './support/dropdown';
import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls } from './support/tauriMock';
import type { LaunchRequest, Snapshot } from '../src/lib/types';

const composer = (page: Page) => page.locator('form.composer');
const rows = (page: Page) => page.locator('.recent-row');
const row = (page: Page, name: string) =>
  rows(page).filter({
    has: page.locator('.recent-name', { hasText: new RegExp(`^${name}$`) }),
  });
const start = (page: Page) =>
  composer(page).getByRole('button', { name: /^(Starten|Installieren …)$/ });

test('the composer starts from the defaults with an empty folder field', async ({
  page,
}) => {
  await page.goto('/');
  const form = composer(page);
  await expect(form.getByRole('radio', { name: 'Claude Code' })).toBeChecked();
  await expect(form.getByRole('radio', { name: 'Lokal' })).toBeChecked();
  await expect(form.getByRole('radio', { name: 'WSL' })).toBeDisabled();
  await expect(
    form.getByRole('radio', { name: 'WSL' }),
  ).toHaveAccessibleDescription('Bald verfügbar');
  const folder = form.getByRole('combobox', { name: 'Ordner oder Repo' });
  await expect(folder).toHaveValue('');
  await expect(folder).toHaveAttribute(
    'placeholder',
    'C:\\Projekte\\app oder owner/repo',
  );
  await expect(
    form.getByRole('button', { name: 'Freigabe: Auto' }),
  ).toBeVisible();
  await expect(start(page)).toHaveText('Starten');
});

test('a start from the composer adds a recent and Enter starts', async ({
  page,
}) => {
  await page.goto('/');
  await expect(rows(page)).toHaveCount(6);
  const form = composer(page);
  await form.getByRole('button', { name: 'Ordner durchsuchen' }).click();
  const folder = form.getByRole('combobox', { name: 'Ordner oder Repo' });
  await expect(folder).toHaveValue('C:\\Users\\demo\\Projekte\\demo-app');
  await form.getByRole('radio', { name: 'Docker' }).check();
  await folder.press('Enter');
  await expect(
    page
      .getByRole('status')
      .filter({ hasText: 'Claude Code gestartet (Demo).' }),
  ).toBeVisible();
  await expect(rows(page)).toHaveCount(7);
  const added = rows(page).first();
  await expect(added.locator('.recent-name')).toHaveText('demo-app');
  await expect(added.getByRole('img', { name: 'Docker' })).toBeVisible();
  await expect(added.locator('.recent-time')).toHaveText('gerade eben');
  // Feedback goes away after 3 s.
  await expect(
    page.getByRole('status').filter({ hasText: 'gestartet' }),
  ).toHaveCount(0, { timeout: 6000 });
});

test('the chips change the next start only; Bypass shows the warning colour', async ({
  page,
}) => {
  await page.goto('/');
  await page
    .getByRole('combobox', { name: 'Ordner oder Repo' })
    .fill('C:\\Users\\demo\\Projekte\\traygent');
  const form = composer(page);
  const permission = form.getByRole('button', { name: /^Freigabe: / });
  await permission.click();
  await expect(permission).toHaveAccessibleName('Freigabe: Bypass');
  await expect(permission).toHaveAttribute('data-warn', 'true');
  await permission.click();
  await expect(permission).toHaveAccessibleName('Freigabe: Nachfragen');
  await choose(form.getByRole('combobox', { name: 'Denken' }), 'Hoch');
  await form.getByRole('combobox', { name: 'Modell' }).fill('opus');
  await choose(form.getByRole('combobox', { name: 'Session' }), 'Fortsetzen');
  await start(page).click();
  const first = rows(page).first();
  await expect(first.locator('.recent-name')).toHaveText('traygent');
  // The options as symbols: the model's letter and three of five bars.
  const model = first.getByRole('img', { name: 'Modell: Opus' });
  await expect(model).toHaveText('O');
  await expect(
    first.getByRole('img', { name: 'Denken: Hoch' }),
  ).toHaveAttribute('data-level', '3');
  await expect(
    first.getByRole('img', { name: 'Freigabe: Nachfragen' }),
  ).toBeVisible();
  // After the start the form begins from the defaults again.
  await expect(permission).toHaveAccessibleName('Freigabe: Auto');
  await expect(form.getByRole('combobox', { name: 'Session' })).toHaveAttribute(
    'data-value',
    'new',
  );
});

test('▶ restarts a recent session with exactly its options', async ({
  page,
}) => {
  await page.goto('/');
  const target = row(page, 'repo-watch');
  await expect(target.locator('.recent-time')).toHaveText('vor 47 Min.');
  await target
    .getByRole('button', { name: 'Codex in repo-watch starten' })
    .click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Codex gestartet (Demo).' }),
  ).toBeVisible();
  const first = rows(page).first();
  await expect(first.locator('.recent-name')).toHaveText('repo-watch');
  await expect(
    first.getByRole('img', { name: 'Modell: gpt-6.1-sol' }),
  ).toHaveText('S');
  await expect(first.getByRole('img', { name: 'Denken: Hoch' })).toBeVisible();
  await expect(first.locator('.recent-time')).toHaveText('gerade eben');
  await expect(rows(page)).toHaveCount(6);
});

test('„Entfernen“ offers „Rückgängig“, which puts the row back', async ({
  page,
}) => {
  await page.goto('/');
  const target = row(page, 'kunden-portal');
  await target
    .getByRole('button', { name: 'Aktionen für kunden-portal' })
    .click();
  const menu = page.getByRole('menu', { name: 'Aktionen für kunden-portal' });
  await expect(menu.getByRole('menuitem')).toHaveText([
    'In Composer übernehmen',
    'Bearbeiten …',
    'Ordner öffnen',
    'Entfernen',
  ]);
  await menu.getByRole('menuitem', { name: 'Entfernen' }).click();
  await expect(rows(page)).toHaveCount(5);
  await expect(row(page, 'kunden-portal')).toHaveCount(0);
  // Focus moved on to the next row, never to <body>.
  await expect(
    row(page, 'agent-tools').getByRole('button', {
      name: 'Aktionen für agent-tools',
    }),
  ).toBeFocused();
  const notice = page
    .getByRole('status')
    .filter({ hasText: 'Aus „Zuletzt“ entfernt.' });
  await notice.getByRole('button', { name: 'Rückgängig' }).click();
  await expect(rows(page)).toHaveCount(6);
  await expect(rows(page).nth(2).locator('.recent-name')).toHaveText(
    'kunden-portal',
  );
  await expect(notice).toHaveCount(0);
});

test('„In Composer übernehmen“ loads the row and focuses „Starten“', async ({
  page,
}) => {
  await page.goto('/');
  await row(page, 'agent-tools')
    .getByRole('button', { name: 'Aktionen für agent-tools' })
    .click();
  await page.getByRole('menuitem', { name: 'In Composer übernehmen' }).click();
  const form = composer(page);
  await expect(form.getByRole('radio', { name: 'OpenCode' })).toBeChecked();
  await expect(form.getByRole('radio', { name: 'Docker' })).toBeChecked();
  await expect(
    form.getByRole('combobox', { name: 'Ordner oder Repo' }),
  ).toHaveValue('C:\\Users\\demo\\Projekte\\agent-tools');
  await expect(form.getByRole('combobox', { name: 'Modell' })).toHaveValue(
    'opencode/big-pickle',
  );
  await expect(start(page)).toBeFocused();
});

test('a missing CLI turns „Starten“ into „Installieren …“ for Settings → Agenten', async ({
  page,
}) => {
  await page.goto('/');
  const form = composer(page);
  await form.getByRole('radio', { name: 'OpenCode' }).check();
  await expect(start(page)).toHaveText('Installieren …');
  await expect(start(page)).not.toHaveAttribute('aria-disabled');
  // In Docker the CLI installs itself on the first start.
  await form.getByRole('radio', { name: 'Docker' }).check();
  await expect(start(page)).toHaveText('Starten');
  await form.getByRole('radio', { name: 'Lokal' }).check();
  await start(page).click();
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await expect(
    dialog.getByRole('tab', { name: 'Agenten', exact: true }),
  ).toHaveAttribute('aria-selected', 'true');
  await expect(dialog.getByRole('radio', { name: 'OpenCode' })).toBeChecked();
  await expect(
    dialog.getByText('Nicht installiert', { exact: true }),
  ).toBeVisible();
  await expect(
    dialog.getByRole('textbox', { name: 'Installationsbefehl' }),
  ).toHaveValue('npm install -g @opencode/cli@latest');
});

test('„Starten“ waits for a folder and explains why in its tooltip', async ({
  page,
}) => {
  await page.goto('/');
  const form = composer(page);
  await form.getByRole('combobox', { name: 'Ordner oder Repo' }).fill('');
  await expect(start(page)).toHaveAttribute('aria-disabled', 'true');
  await expect(start(page)).toHaveAttribute('title', 'Ordner wählen');
  // aria-disabled keeps it focusable; a forced click starts nothing.
  await start(page).click({ force: true });
  await expect(rows(page)).toHaveCount(6);
  await form.getByRole('button', { name: 'Zuletzt verwendete Ordner' }).click();
  await page.getByRole('menuitem', { name: /^Notizen/ }).click();
  await expect(
    form.getByRole('combobox', { name: 'Ordner oder Repo' }),
  ).toHaveValue('C:\\Users\\demo\\Notizen');
  await expect(start(page)).not.toHaveAttribute('aria-disabled');
});

test('natively a start sends the exact request and shows the German error', async ({
  page,
}) => {
  const mock = await installTauriMock<Snapshot, { fail: boolean }>(
    page,
    (snapshot, state) => async (cmd, args) => {
      if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
      if (cmd === 'launch_session') {
        if (state.fail) throw 'Ordner existiert nicht: C:\\Users\\demo\\weg';
        const request = args?.request as LaunchRequest;
        const recent = {
          ...snapshot.recents[0]!,
          options: request.options,
          lastStartedAt: new Date().toISOString(),
        };
        return {
          recent,
          snapshot: {
            ...snapshot,
            recents: [recent, ...snapshot.recents.slice(1)],
          },
        };
      }
      return null;
    },
    { arg: nativeSnapshot(), state: { fail: false } },
  );
  await page.goto('/');
  await page
    .getByRole('combobox', { name: 'Ordner oder Repo' })
    .fill('C:\\Users\\demo\\Projekte\\traygent');
  await start(page).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Claude Code gestartet.' }),
  ).toBeVisible();
  const calls = await readCalls(page, 'launch_session');
  expect(calls.map((call) => call.args)).toEqual([
    {
      request: {
        agent: 'claude',
        env: 'local',
        folder: 'C:\\Users\\demo\\Projekte\\traygent',
        options: {
          model: 'default',
          effort: 'default',
          permission: 'auto',
          extraArgs: '',
        },
        session: 'new',
        ports: [],
      },
    },
  ]);
  await mock.configure({ fail: true });
  await start(page).click();
  await expect(page.getByRole('alert')).toHaveText(
    'Ordner existiert nicht: C:\\Users\\demo\\weg',
  );
});

test('the native default snapshot (Rust golden fixture) renders', async ({
  page,
}) => {
  const fixture = JSON.parse(
    readFileSync(join(process.cwd(), 'fixtures/snapshot.default.json'), 'utf8'),
  ) as Snapshot;
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd) =>
      cmd === 'snapshot' || cmd === 'refresh_detection' ? snapshot : null,
    { arg: fixture, state: {} },
  );
  await page.goto('/');
  const form = composer(page);
  for (const entry of fixture.catalog)
    await expect(form.getByRole('radio', { name: entry.label })).toBeVisible();
  await expect(page.getByText('Noch keine Sessions')).toHaveCount(
    fixture.recents.length ? 0 : 1,
  );
  await expect(page.locator('.app-footer .demo-tag')).toHaveText('Lokal');
  await expect(page.locator('.app-footer .docker-state')).toBeVisible();
});
