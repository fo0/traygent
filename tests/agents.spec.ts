import { choose } from './support/dropdown';
import { expect, test, type Page } from './support/test';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCalls, readCommands } from './support/tauriMock';
import type { EnvStatus, LaunchRequest, Snapshot } from '../src/lib/types';

/*
 * Probes, option rules and the install flow, in the labelled demo and against
 * the labelled native-IPC mock (synthetic snapshots only).
 */
const composer = (page: Page) => page.locator('form.composer');
const start = (page: Page) =>
  composer(page).getByRole('button', { name: /^(Starten|Installieren …)$/ });
const row = (page: Page, name: string) =>
  page.locator('.recent-row').filter({
    has: page.locator('.recent-name', { hasText: new RegExp(`^${name}$`) }),
  });
const settingsDialog = (page: Page) =>
  page.getByRole('dialog', { name: 'Einstellungen' });
async function openSettings(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  return settingsDialog(page);
}

/** Serves one snapshot natively and records every call. */
function serve(page: Page, snapshot: Snapshot) {
  return installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd, args) => {
      if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
      if (cmd === 'launch_session') {
        const request = args?.request as LaunchRequest;
        return { recent: { ...snapshot.recents[0]!, ...request }, snapshot };
      }
      if (cmd === 'autostart_status')
        return { status: 'disabled', registered: false };
      return null;
    },
    { arg: snapshot, state: {} },
  );
}

const WINDOWS =
  'Docker nutzt Windows-Container – auf Linux-Container umschalten';

test('Docker in Windows-container mode stays selectable and names the reason', async ({
  page,
}) => {
  const base = nativeSnapshot();
  const docker: EnvStatus = {
    env: 'docker',
    state: 'unavailable',
    version: '27.5.1',
    detail: WINDOWS,
  };
  await serve(page, {
    ...base,
    environments: base.environments.map((env) =>
      env.env === 'docker' ? docker : env,
    ),
    agents: base.agents.map((status) =>
      status.env === 'docker'
        ? { agent: status.agent, env: 'docker', state: 'unavailable' }
        : status.agent === 'claude'
          ? { ...status, detail: 'Git for Windows empfohlen' }
          : status,
    ),
  });
  await page.goto('/');
  const form = composer(page);
  const radio = form.getByRole('radio', { name: 'Docker' });
  await expect(radio).toBeEnabled();
  await expect(radio).toHaveAccessibleDescription(
    `Docker nicht verfügbar · ${WINDOWS}`,
  );
  await expect(form.getByRole('radio', { name: 'WSL' })).toBeDisabled();
  await radio.check();
  await expect(start(page)).toHaveAttribute('aria-disabled', 'true');
  await expect(start(page)).toHaveAttribute('title', WINDOWS);
  await expect(
    row(page, 'kunden-portal').getByRole('button', { name: /starten$/ }),
  ).toHaveAttribute('title', WINDOWS);
  await expect(page.locator('.app-footer .docker-state')).toHaveText(
    'Docker nicht verfügbar',
  );

  // Settings → Agenten: each probe on one line, its note included.
  const dialog = await openSettings(page);
  const lines = dialog.locator('.status-lines dd');
  await expect(lines.nth(0)).toHaveText('2.1.317 · Git for Windows empfohlen');
  await expect(lines.nth(0)).toHaveAttribute(
    'title',
    /^2\.1\.317 · Git for Windows empfohlen · C:\\/,
  );
  await expect(lines.nth(1)).toHaveText(`Docker nicht verfügbar · ${WINDOWS}`);
  await expect(lines.nth(1)).toHaveAttribute(
    'title',
    `Docker nicht verfügbar · ${WINDOWS}`,
  );
});

test('a read-only state file shows a warning that stays', async ({ page }) => {
  const notice =
    'Die Einstellungen stammen aus einer neueren Traygent-Version und werden nicht überschrieben. Bitte Traygent aktualisieren.';
  await page.clock.install();
  await serve(page, nativeSnapshot({ notice }));
  await page.goto('/');
  const banner = page.getByRole('status').filter({ hasText: notice });
  await expect(banner).toBeVisible();
  await expect(banner.getByRole('button')).toHaveCount(0);
  await page.clock.fastForward(10_000);
  await expect(banner).toBeVisible();
});

test('OpenCode thinks only with a model: no effort is shown, sent or recorded without one', async ({
  page,
}) => {
  await page.goto('/');
  await page
    .getByRole('combobox', { name: 'Ordner oder Repo' })
    .fill('C:\\Users\\demo\\Projekte\\traygent');
  const form = composer(page);
  await form.getByRole('radio', { name: 'OpenCode' }).check();
  await form.getByRole('radio', { name: 'Docker' }).check();
  const effort = form.getByRole('combobox', { name: 'Denken' });
  const model = form.getByRole('combobox', { name: 'Modell' });
  await expect(effort).toHaveCount(0);
  await model.fill('opencode/big-pickle');
  await choose(effort, 'Hoch');
  await model.fill('');
  await expect(effort).toHaveCount(0);
  await start(page).click();
  const first = page.locator('.recent-row').first();
  await expect(first.getByRole('img', { name: 'Docker' })).toBeVisible();
  // Neither a model badge nor thinking bars: OpenCode sends neither here.
  await expect(
    first.getByRole('img', { name: /^(Modell|Denken): / }),
  ).toHaveCount(0);

  const dialog = await openSettings(page);
  await dialog.getByRole('radio', { name: 'OpenCode' }).check();
  const defaultEffort = dialog.getByRole('combobox', { name: 'Denken' });
  await expect(defaultEffort).toBeDisabled();
  await expect(defaultEffort).toHaveAttribute(
    'title',
    'Denken: nur mit gewähltem Modell',
  );
  await dialog
    .getByRole('combobox', { name: 'Modell' })
    .fill('opencode/big-pickle');
  await expect(defaultEffort).toBeEnabled();
  await expect(defaultEffort).toHaveAttribute(
    'title',
    'Denken: Standard – Nur mit gewähltem Modell (Modell-Variante)',
  );
});

test('the catalog hints of model and effort live in the tooltips only', async ({
  page,
}) => {
  await page.goto('/');
  const form = composer(page);
  await expect(form.locator('label.chip-field')).toHaveAttribute(
    'title',
    'Modell: Standard – Ohne --model: Standard des Kontos',
  );
  const effort = form.getByRole('combobox', { name: 'Denken' });
  await expect(effort).toHaveAttribute(
    'title',
    'Denken: Standard – Ohne --effort: Standard des Modells',
  );
  await expect(form.getByText('Ohne --model', { exact: false })).toHaveCount(0);
  await form.getByRole('combobox', { name: 'Modell' }).fill('haiku');
  await expect(form.locator('label.chip-field')).toHaveAttribute(
    'title',
    'Modell: Haiku – Schnell und günstig',
  );
  const dialog = await openSettings(page);
  await expect(
    dialog.getByRole('combobox', { name: 'Modell' }),
  ).toHaveAttribute(
    'title',
    'Modell: Standard – Ohne --model: Standard des Kontos',
  );
});

test('a restart names the extra arguments it passes', async ({ page }) => {
  const base = nativeSnapshot();
  const args = '--search --profile work';
  const recent = {
    ...base.recents[1]!,
    options: { ...base.recents[1]!.options, extraArgs: args },
  };
  await serve(page, {
    ...base,
    recents: [base.recents[0]!, recent, ...base.recents.slice(2)],
  });
  await page.goto('/');
  const target = row(page, 'repo-watch');
  const restart = target.getByRole('button', {
    name: `Codex in repo-watch starten · Extra-Argumente: ${args}`,
    exact: true,
  });
  // The start covers the row, so its tooltip names everything the row shows.
  const literal = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  await expect(restart).toHaveAttribute(
    'title',
    new RegExp(
      `^Codex erneut starten · ${literal(recent.folder)} · .* · Extra-Argumente: ${literal(args)} · \\d\\d\\.`,
    ),
  );
  // Assistive tech hears the arguments once, in the name; the description
  // is the folder alone, not the long tooltip.
  await expect(restart).toHaveAccessibleDescription(recent.folder);
  await restart.click();
  await expect
    .poll(async () =>
      (await readCalls(page, 'launch_session')).map(
        (call) => (call.args?.request as LaunchRequest).options.extraArgs,
      ),
    )
    .toEqual([args]);
});

test('„Installieren …“ closes Settings once the terminal opened and detects nothing yet', async ({
  page,
}) => {
  await serve(page, nativeSnapshot());
  await page.goto('/');
  await composer(page).getByRole('radio', { name: 'OpenCode' }).check();
  await expect(start(page)).toHaveText('Installieren …');
  await start(page).click();
  const dialog = settingsDialog(page);
  await dialog
    .getByRole('button', { name: 'Installieren', exact: true })
    .click();
  await expect(dialog).toHaveCount(0);
  const note = page.getByRole('status').filter({
    hasText: 'Installation gestartet – danach aktualisieren (F5).',
  });
  await expect(note).toBeVisible();
  const commands = await readCommands(page);
  expect(commands.slice(commands.indexOf('install_agent'))).toEqual([
    'install_agent',
  ]);
  expect(
    (await readCalls(page, 'install_agent')).map((call) => call.args),
  ).toEqual([{ agent: 'opencode' }]);
  await expect(note).toHaveCount(0, { timeout: 6000 });
});

test('the install flow keeps Settings open while the local status names a reason', async ({
  page,
}) => {
  const base = nativeSnapshot();
  // Synthetic: what detection reports for an OpenCode 1 npm install.
  const detail =
    'OpenCode 1 gefunden – zuerst „npm uninstall -g opencode-ai“ ausführen, dann V2 installieren';
  await serve(page, {
    ...base,
    agents: base.agents.map((status) =>
      status.agent === 'opencode' && status.env === 'local'
        ? { ...status, version: '1.4.2', detail }
        : status,
    ),
  });
  await page.goto('/');
  await composer(page).getByRole('radio', { name: 'OpenCode' }).check();
  await expect(start(page)).toHaveText('Installieren …');
  await start(page).click();
  const dialog = settingsDialog(page);
  await dialog
    .getByRole('button', { name: 'Installieren', exact: true })
    .click();
  await expect(
    dialog.getByRole('status').filter({
      hasText: 'Installation gestartet – danach aktualisieren (F5).',
    }),
  ).toBeVisible();
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText(detail)).toBeVisible();
  expect(
    (await readCalls(page, 'install_agent')).map((call) => call.args),
  ).toEqual([{ agent: 'opencode' }]);
});

test('the install flow keeps Settings open while something is staged', async ({
  page,
}) => {
  await page.goto('/');
  await composer(page).getByRole('radio', { name: 'OpenCode' }).check();
  await start(page).click();
  const dialog = settingsDialog(page);
  await dialog
    .getByRole('radiogroup', { name: 'Freigabe' })
    .getByRole('radio', { name: 'YOLO' })
    .check();
  await dialog
    .getByRole('button', { name: 'Installieren', exact: true })
    .click();
  await expect(
    dialog
      .getByRole('status')
      .filter({ hasText: 'Demo: kein Terminal geöffnet.' }),
  ).toBeVisible();
  await expect(dialog).toBeVisible();
});

test('agents, Docker and GitHub carry their product marks; the name stays accessible', async ({
  page,
}) => {
  await page.goto('/');
  // „Zuletzt“: the badge is an image named after the agent, drawn as its mark.
  const badge = row(page, 'traygent').getByRole('img', { name: 'Claude Code' });
  await expect(badge.locator('svg.mark-claude')).toBeVisible();
  await expect(
    row(page, 'agent-tools').locator('.agent-badge svg.mark-opencode'),
  ).toBeVisible();
  await expect(
    row(page, 'repo-watch').getByRole('img', { name: /^GitHub-Repository / }),
  ).toBeVisible();
  // The composer's agent radios keep their label beside the mark.
  await expect(
    composer(page).getByRole('radio', { name: 'Codex' }),
  ).toBeAttached();
  await expect(
    composer(page).locator(
      'label.segment:has(input[value="codex"]) svg.mark-codex',
    ),
  ).toBeVisible();
  await expect(page.locator('.app-footer svg.mark-docker')).toBeVisible();
  // Settings tabs lead with an icon; the tab name is the label only.
  const dialog = await openSettings(page);
  await expect(dialog.getByRole('tab', { name: 'GitHub' })).toBeVisible();
  await expect(
    dialog.getByRole('tab', { name: 'GitHub' }).locator('svg.mark-github'),
  ).toBeVisible();
});

test('the install command copy button announces its result', async ({
  page,
}) => {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/');
  const dialog = await openSettings(page);
  await dialog
    .getByRole('button', { name: 'Installationsbefehl kopieren' })
    .click();
  await expect(
    dialog.getByRole('status').filter({ hasText: /^Kopiert$/ }),
  ).toBeAttached();
  await expect(dialog.getByRole('button', { name: 'Kopiert' })).toBeVisible();
});
