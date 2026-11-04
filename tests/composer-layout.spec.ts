import { expect, test, type Page } from './support/test';
import { LAYOUT_FONTS, pinFont, type LayoutFont } from './support/fonts';

// The composer's sizes: the leading agent row, „Starten“ and the chip row.

const composer = (page: Page) => page.locator('form.composer');
const agents = (page: Page) =>
  composer(page).getByRole('radiogroup', { name: 'Agent' });
const envs = (page: Page) =>
  composer(page).getByRole('radiogroup', { name: 'Umgebung' });
const segmentHeight = (group: ReturnType<typeof agents>) =>
  group
    .locator('.segment')
    .first()
    .evaluate((node) => node.getBoundingClientRect().height);

/** Widths at which every agent name fits; DejaVu cuts at the 360 px minimum. */
const UNCUT: Record<LayoutFont, number[]> = {
  'Liberation Sans': [480, 440, 360],
  'DejaVu Sans': [480, 440],
};

test('the agent row leads the composer: 36 px, 14.5 px names, 18 px marks at 480 px', async ({
  page,
}) => {
  await page.setViewportSize({ width: 480, height: 760 });
  await page.goto('/');
  const segment = agents(page).locator('.segment').first();
  expect(await segmentHeight(agents(page))).toBe(36);
  await expect(segment).toHaveCSS('font-size', '14.5px');
  await expect(segment.locator('svg')).toHaveCSS('width', '18px');
  expect(await segmentHeight(envs(page))).toBe(28);
  // Below the default window the row keeps the regular size.
  for (const width of [479, 360]) {
    await page.setViewportSize({ width, height: 760 });
    expect(await segmentHeight(agents(page))).toBe(
      await segmentHeight(envs(page)),
    );
  }
});

for (const font of LAYOUT_FONTS) {
  test(`no agent name is cut in ${font}`, async ({ page }) => {
    await page.goto('/');
    await pinFont(page, font);
    for (const width of UNCUT[font]) {
      await page.setViewportSize({ width, height: 760 });
      const cut = await agents(page).evaluate((group) =>
        [...group.querySelectorAll('.segment-label')]
          .filter((label) => label.scrollWidth > label.clientWidth)
          .map((label) => label.textContent),
      );
      expect(cut, `${width}px`).toEqual([]);
    }
  });

  test(`every agent's chips keep one row beside „Starten“ at 480 px in ${font}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 480, height: 760 });
    await page.goto('/');
    await pinFont(page, font);
    const form = composer(page);
    for (const agent of ['Claude Code', 'OpenCode', 'Codex'])
      for (const env of ['Lokal', 'Docker']) {
        await form.getByRole('radio', { name: agent }).check();
        await form.getByRole('radio', { name: env }).check();
        await expect(form.getByRole('button', { name: /^Ports/ })).toHaveCount(
          env === 'Docker' ? 1 : 0,
        );
        const layout = await form.evaluate((node) => {
          const chips = [...node.querySelectorAll('.option-chips .chip')]
            .map((chip) => chip.getBoundingClientRect())
            .filter((rect) => rect.width > 0);
          const button = node
            .querySelector('.composer-start')!
            .getBoundingClientRect();
          return {
            rows: new Set(chips.map((rect) => Math.round(rect.top))).size,
            chipHeight: chips[0]!.height,
            chipBottom: Math.round(chips[0]!.bottom),
            startHeight: button.height,
            startBottom: Math.round(button.bottom),
          };
        });
        const where = `${agent} · ${env}`;
        expect(layout.rows, where).toBe(1);
        // „Starten“ is about the chips' height and ends level with them.
        expect(layout.startHeight, where).toBe(30);
        expect(layout.startHeight - layout.chipHeight, where).toBeLessThan(3);
        expect(layout.startBottom, where).toBe(layout.chipBottom);
      }
  });
}

test('the permission chip is a symbol; its label is the name and the tooltip', async ({
  page,
}) => {
  await page.goto('/');
  const chip = composer(page).getByRole('button', { name: /^Freigabe: / });
  await expect(chip).toHaveAccessibleName('Freigabe: Auto');
  await expect(chip).toHaveText('');
  await expect(chip).toHaveAttribute('title', /^Freigabe: Auto/);
  await expect(chip.locator('svg')).toHaveCount(1);
});
