import { expect, test } from './support/test';
import { pinFont } from './support/fonts';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock, readCommands } from './support/tauriMock';
import type { Snapshot } from '../src/lib/types';

test('the browser preview shows the labelled demo dashboard', async ({
  page,
}) => {
  await page.goto('/');
  const tray = page.getByRole('region', { name: 'Traygent' });
  await expect(
    tray.getByRole('heading', { name: 'Traygent', level: 1 }),
  ).toBeVisible();
  // The composer sits right under the app bar; its section has no heading.
  await expect(
    tray.getByRole('region', { name: 'Neue Session' }),
  ).toBeVisible();
  await expect(tray.getByRole('heading', { name: 'Neue Session' })).toHaveCount(
    0,
  );
  await expect(
    tray.getByRole('heading', { name: 'Zuletzt', level: 2 }),
  ).toBeVisible();
  await expect(tray.locator('.recent-row')).toHaveCount(6);
  await expect(page.locator('.app-footer .demo-tag')).toHaveText('Demo');
  await expect(page.locator('.app-footer .docker-state')).toHaveText(
    'Docker 27.5 · bereit',
  );
});

test('F5 refreshes once without reloading; the footer shows the time', async ({
  page,
}) => {
  await page.goto('/');
  const footer = page.locator('.app-footer');
  const stand = footer.getByRole('button', {
    name: /^Aktualisieren · Stand \d\d:\d\d$/,
  });
  await expect(stand).toBeEnabled();
  await page.evaluate(() => ((window as { marker?: number }).marker = 1));
  await page.keyboard.press('F5');
  // The detection answers at once and spins until its result is pushed.
  const refresh = page.getByRole('button', {
    name: 'Aktualisieren',
    exact: true,
  });
  await expect(refresh).toHaveClass(/spinning/);
  await expect(refresh).not.toHaveClass(/spinning/);
  await expect(stand).toBeVisible();
  // A webview reload would have dropped the marker.
  expect(
    await page.evaluate(() => (window as { marker?: number }).marker),
  ).toBe(1);
  await expect(
    page.getByRole('status').filter({ hasText: 'Demo aktualisiert.' }),
  ).toBeVisible();
});

test('settings open on „Agenten“ and „Abbrechen“ closes without saving', async ({
  page,
}) => {
  await page.goto('/');
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await expect(dialog.getByRole('tab')).toHaveText([
    'Agenten',
    'Umgebung',
    'GitHub',
    'App',
    'Updates',
  ]);
  await expect(
    dialog.getByRole('tab', { name: 'Agenten', exact: true }),
  ).toHaveAttribute('aria-selected', 'true');
  await dialog.getByRole('tab', { name: 'App', exact: true }).click();
  await expect(
    dialog.getByRole('radiogroup', { name: 'Darstellung' }),
  ).toBeVisible();
  await expect(dialog.getByRole('region', { name: 'Autostart' })).toBeVisible();
  await expect(dialog.getByRole('region', { name: 'App-Update' })).toHaveCount(
    0,
  );
  await dialog.getByRole('tab', { name: 'Updates', exact: true }).click();
  await expect(
    dialog.getByRole('region', { name: 'App-Update' }),
  ).toBeVisible();
  await expect(
    dialog.getByRole('button', { name: 'Speichern', exact: true }),
  ).toHaveAttribute('aria-disabled', 'true');
  await dialog.getByRole('button', { name: 'Abbrechen', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByRole('button', { name: 'Einstellungen', exact: true }),
  ).toBeFocused();
});

// 480 px, the default window, is the narrowest width that still shows the
// tab icons; 479 px is the widest without them.
for (const width of [360, 479, 480]) {
  test(`all settings tabs fit side by side at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 760 });
    await page.goto('/');
    await page
      .getByRole('button', { name: 'Einstellungen', exact: true })
      .click();
    const strip = page.getByRole('tablist', { name: 'Einstellungsbereiche' });
    await expect(strip.getByRole('tab')).toHaveCount(5);
    const spills = await strip.evaluate((node) => [
      ...(node.scrollWidth > node.clientWidth ? ['strip'] : []),
      ...[...node.querySelectorAll('[role=tab]')]
        .filter((tab) => tab.scrollWidth > tab.clientWidth)
        .map((tab) => tab.textContent),
    ]);
    expect(spills).toEqual([]);
  });
}

test('the tab strip sits centred; settings controls share one height, list rows another', async ({
  page,
}) => {
  await page.setViewportSize({ width: 480, height: 900 });
  await page.goto('/');
  // A one-line account row needs a font of Segoe UI's width class.
  await pinFont(page, 'Liberation Sans');
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  for (const name of ['Agenten', 'Umgebung', 'GitHub', 'App', 'Updates']) {
    await dialog.getByRole('tab', { name: new RegExp(`^${name}`) }).click();
    const layout = await dialog.evaluate((node) => {
      const box = (selector: string) =>
        node.querySelector(selector)!.getBoundingClientRect();
      const heading = box('.modal-heading');
      const strip = box('.settings-tabs');
      const tab = box('.settings-tabs [aria-selected="true"]');
      const panel = node.querySelector('#settings-panel')!;
      const heights = (selectors: string[]) => [
        ...new Set(
          [...panel.querySelectorAll(selectors.join(', '))]
            .filter((item) => item.getClientRects().length)
            .map((item) => Math.round(item.getBoundingClientRect().height)),
        ),
      ];
      return {
        above: tab.top - heading.bottom,
        // The strip's bottom edge is its 1 px divider.
        below: strip.bottom - 1 - tab.bottom,
        controls: heights([
          '.segmented',
          '.text-field',
          '.theme-option',
          '.status-lines > div',
          '.github-account',
          ':is(.primary-button, .secondary-button, .danger-button)',
        ]),
        rows: heights(['.env-toggle', '.port-preset', '.local-account']),
      };
    });
    expect(layout.above, name).toBe(layout.below);
    expect(layout.above, name).toBeGreaterThan(0);
    if (layout.controls.length) expect(layout.controls, name).toEqual([32]);
    if (layout.rows.length) expect(layout.rows, name).toEqual([28]);
  }
  // The footer's buttons take the same height.
  const footer = await dialog
    .locator('.modal-actions button')
    .evaluateAll((items) =>
      items.map((item) => item.getBoundingClientRect().height),
    );
  expect(footer).toEqual([32, 32]);
});

test('the native shell is never labelled demo', async ({ page }) => {
  await installTauriMock<Snapshot, Record<string, never>>(
    page,
    (snapshot) => async (cmd) => {
      if (cmd === 'snapshot' || cmd === 'refresh_detection') return snapshot;
      if (cmd === 'check_update') return null;
      return null;
    },
    { arg: nativeSnapshot(), state: {} },
  );
  await page.goto('/');
  await expect(page.locator('.app-footer .demo-tag')).toHaveText('Lokal');
  await expect(page.getByText('Demo', { exact: true })).toHaveCount(0);
  await expect.poll(() => readCommands(page)).toContain('check_update');
});

for (const width of [360, 480]) {
  test(`compact panel keeps readable text and click targets at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 760 });
    await page.goto('/');
    await expect(page.locator('.recent-row')).toHaveCount(6);
    const geometry = await page.evaluate(() => {
      const inside = (outer: Element, inner: Element | null) => {
        if (!inner) return false;
        const a = outer.getBoundingClientRect();
        const b = inner.getBoundingClientRect();
        return b.left >= a.left - 0.5 && b.right <= a.right + 0.5;
      };
      const header = document
        .querySelector('.app-bar')!
        .getBoundingClientRect();
      const footer = document
        .querySelector('.app-footer')!
        .getBoundingClientRect();
      const status = document
        .querySelector('.connection')!
        .getBoundingClientRect();
      return {
        headerHeight: header.height,
        statusInside:
          status.top >= footer.top &&
          status.bottom <= footer.bottom &&
          status.right <= footer.right,
        bodyFont: parseFloat(getComputedStyle(document.body).fontSize),
        minIconTarget: Math.min(
          ...[...document.querySelectorAll('.icon-button')].map((button) =>
            Math.min(
              button.getBoundingClientRect().width,
              button.getBoundingClientRect().height,
            ),
          ),
        ),
        overflow: document.documentElement.scrollWidth > innerWidth,
        composerFits: [
          ...document.querySelectorAll('.composer > *, .composer-actions > *'),
        ].every((item) => inside(document.querySelector('.composer')!, item)),
        rowsFit: [...document.querySelectorAll('.recent-row')].every(
          (row) =>
            inside(row, row.querySelector('.recent-more')) &&
            row.querySelector<HTMLElement>('.recent-name')!.offsetWidth > 40,
        ),
        startRightEdge:
          Math.abs(
            document.querySelector('.composer-start')!.getBoundingClientRect()
              .right -
              document
                .querySelector('.composer-actions')!
                .getBoundingClientRect().right,
          ) < 1,
      };
    });
    expect(geometry.headerHeight).toBeLessThanOrEqual(56);
    expect(geometry.statusInside).toBe(true);
    expect(geometry.bodyFont).toBeGreaterThanOrEqual(12);
    expect(geometry.minIconTarget).toBeGreaterThanOrEqual(24);
    expect(geometry.overflow).toBe(false);
    expect(geometry.composerFits).toBe(true);
    expect(geometry.rowsFit).toBe(true);
    expect(geometry.startRightEdge).toBe(true);
  });
}
