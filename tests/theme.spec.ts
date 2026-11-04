import { expect, test, type Page } from './support/test';

/** The panel surface (--surface) of each theme. */
const SURFACE = {
  light: 'rgb(246, 246, 248)',
  dark: 'rgb(17, 17, 20)',
} as const;
type Scheme = keyof typeof SURFACE;

/** Asserts the rendered theme: surface colour and `color-scheme`. */
async function expectTheme(page: Page, scheme: Scheme) {
  await expect(page.locator('.tray')).toHaveCSS(
    'background-color',
    SURFACE[scheme],
  );
  expect(
    await page.evaluate(
      () => getComputedStyle(document.documentElement).colorScheme,
    ),
  ).toBe(scheme);
}

/** Opens Einstellungen → App and returns the „Darstellung“ radio group. */
async function themeSetting(page: Page) {
  await page
    .getByRole('button', { name: 'Einstellungen', exact: true })
    .click();
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await dialog.getByRole('tab', { name: 'App', exact: true }).click();
  return dialog.getByRole('radiogroup', { name: 'Darstellung' });
}

const option = (group: ReturnType<Page['locator']>, name: string) =>
  group.getByRole('radio', { name, exact: true });

/** „Speichern“ keeps the previewed theme and closes the dialog. */
async function save(page: Page) {
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await dialog.getByRole('button', { name: 'Speichern', exact: true }).click();
  await expect(dialog).toHaveCount(0);
}

const stored = (page: Page) =>
  page.evaluate(() => localStorage.getItem('traygent.theme.v1'));

for (const scheme of ['light', 'dark'] as const) {
  test(`„System“ renders the ${scheme} theme the OS prefers`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto('/');
    await expectTheme(page, scheme);
    await expect(page.locator('html')).not.toHaveAttribute('data-theme');
    await expect(option(await themeSetting(page), 'System')).toBeChecked();
  });
}

test('„Dunkel“ wins over a light OS at once and after a reload', async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/');
  await expectTheme(page, 'light');
  const group = await themeSetting(page);
  await option(group, 'Dunkel').check();
  await expectTheme(page, 'dark');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  // A preview until „Speichern“.
  expect(await stored(page)).toBeNull();
  await save(page);
  expect(await stored(page)).toBe('"dark"');
  // The browser frame follows the choice, not the OS.
  await expect(
    page.locator('meta[name="theme-color"][data-scheme="dark"]'),
  ).toHaveAttribute('media', 'all');
  await expect(
    page.locator('meta[name="theme-color"][data-scheme="light"]'),
  ).toHaveAttribute('media', 'not all');

  await page.reload();
  await expectTheme(page, 'dark');
  await expect(option(await themeSetting(page), 'Dunkel')).toBeChecked();
});

test('„Hell“ wins over a dark OS; „System“ follows the OS again', async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/');
  let group = await themeSetting(page);
  await option(group, 'Hell').check();
  await expectTheme(page, 'light');
  await save(page);

  await page.reload();
  await expectTheme(page, 'light');
  group = await themeSetting(page);
  await expect(option(group, 'Hell')).toBeChecked();
  await option(group, 'System').check();
  await expectTheme(page, 'dark');
  await expect(page.locator('html')).not.toHaveAttribute('data-theme');
  await save(page);
  expect(await stored(page)).toBeNull();

  await page.reload();
  await expectTheme(page, 'dark');
});

test('only „System“ follows a live change of the OS preference', async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/');
  await page.emulateMedia({ colorScheme: 'light' });
  await expectTheme(page, 'light');

  await option(await themeSetting(page), 'Dunkel').check();
  await save(page);
  await page.emulateMedia({ colorScheme: 'dark' });
  await expectTheme(page, 'dark');
  await page.emulateMedia({ colorScheme: 'light' });
  await expectTheme(page, 'dark');
});

test('the theme radios work with the arrow keys', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/');
  const group = await themeSetting(page);
  await option(group, 'System').focus();
  await page.keyboard.press('ArrowRight');
  await expect(option(group, 'Hell')).toBeChecked();
  await expect(option(group, 'Hell')).toBeFocused();
  await expectTheme(page, 'light');
});

test('an unknown stored theme falls back to „System“', async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem('traygent.theme.v1', JSON.stringify('sepia')),
  );
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/');
  await expectTheme(page, 'light');
  await expect(page.locator('html')).not.toHaveAttribute('data-theme');
  await expect(option(await themeSetting(page), 'System')).toBeChecked();
});

test('a choice that cannot be saved applies and stays shown until restart', async ({
  page,
}) => {
  await page.addInitScript(() => {
    const setItem = Storage.prototype.setItem;
    Storage.prototype.setItem = function (key: string, value: string) {
      if (key === 'traygent.theme.v1')
        throw new DOMException('denied', 'SecurityError');
      setItem.call(this, key, value);
    };
  });
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/');
  await option(await themeSetting(page), 'Hell').check();
  await expectTheme(page, 'light');
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await dialog.getByRole('button', { name: 'Speichern', exact: true }).click();
  // The dialog stays open with one error line; the theme stays applied.
  await expect(dialog.getByRole('alert')).toHaveText(
    'Darstellung ließ sich nicht speichern und gilt bis zum nächsten Start.',
  );
  expect(await stored(page)).toBeNull();

  await page.getByRole('button', { name: 'Dialog schließen' }).click();
  await expect(option(await themeSetting(page), 'Hell')).toBeChecked();
  await expectTheme(page, 'light');
});

for (const scheme of ['light', 'dark'] as const) {
  test(`the dashboard reads the ${scheme} tokens: Bypass in the warning colour`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto('/');
    await expectTheme(page, scheme);
    const token = (name: string) =>
      page.evaluate((name) => {
        const probe = document.createElement('span');
        probe.style.color = `var(${name})`;
        document.body.append(probe);
        const value = getComputedStyle(probe).color;
        probe.remove();
        return value;
      }, name);
    const permission = page
      .locator('form.composer')
      .getByRole('button', { name: /^Freigabe: / });
    await permission.click();
    await expect(permission).toHaveAccessibleName('Freigabe: Bypass');
    await expect(permission).toHaveCSS('color', await token('--wait'));
    // The composer card and the rows sit on the raised surface.
    await expect(page.locator('form.composer')).toHaveCSS(
      'background-color',
      await token('--raised'),
    );
    await expect(
      page.locator('.recent-row').first().locator('.recent-name'),
    ).toHaveCSS('color', await token('--text'));
  });
}
