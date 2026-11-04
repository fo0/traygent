import { expect, test, type Page } from './support/test';

/*
 * „Zuletzt“ keeps its fixed-size parts in fixed columns (owner request,
 * #20): repo, environment, model, thinking, permission and time line up from
 * row to row, whichever parts a row has. The demo's recents mix rows with
 * and without a repository, a model, thinking bars and ports.
 */
const columns = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll('.recent-row')].map((row) => {
      const meta = row.querySelector('.recent-meta')!;
      const time = meta.querySelector<HTMLElement>('.recent-time')!;
      return {
        // Every column after the ports chip: its left and right edge; the
        // right-aligned time by its right edge.
        cells: [...meta.children].slice(1).map((cell) => {
          const box = cell.getBoundingClientRect();
          return cell === time
            ? [Math.round(box.right)]
            : [Math.round(box.left), Math.round(box.right)];
        }),
        permission: Math.round(
          meta.querySelector('.recent-permission')!.getBoundingClientRect()
            .left,
        ),
        timeFits: time.scrollWidth <= time.clientWidth,
        nameWidth: row.querySelector<HTMLElement>('.recent-name')!.offsetWidth,
      };
    }),
  );

for (const width of [480, 360]) {
  test(`every icon of „Zuletzt“ sits in the same column at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 760 });
    await page.goto('/');
    await expect(page.locator('.recent-row')).toHaveCount(6);
    // Rows that differ in what they show: a repository, a model, ports.
    await expect(page.locator('.recent-repo')).toHaveCount(1);
    await expect(page.locator('.recent-ports')).toHaveCount(1);
    await expect(page.locator('.recent-row .model-badge')).toHaveCount(3);
    const rows = await columns(page);
    const [first, ...rest] = rows;
    expect(first!.cells).toHaveLength(6);
    for (const row of rest) {
      expect(row.cells).toEqual(first!.cells);
      expect(row.permission).toBe(first!.permission);
    }
    expect(rows.every((row) => row.timeFits)).toBe(true);
    expect(rows.every((row) => row.nameWidth > 40)).toBe(true);
  });
}

test('an absent part keeps its column as a hidden, unnamed slot', async ({
  page,
}) => {
  await page.goto('/');
  const row = page.locator('.recent-row').first();
  await expect(row.locator('.recent-name')).toHaveText('traygent');
  // No repository, no ports, the default model: three empty slots.
  const slots = row.locator('.recent-slot');
  await expect(slots).toHaveCount(3);
  for (const slot of await slots.all()) {
    await expect(slot).toHaveAttribute('aria-hidden', 'true');
    await expect(slot).not.toHaveAttribute('role');
  }
  // The model badge is an 18 px tile.
  const badge = page.getByRole('img', { name: 'Modell: Opus' });
  expect(await badge.boundingBox()).toMatchObject({ width: 18, height: 18 });
});
