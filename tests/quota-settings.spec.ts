import { expect, test } from './support/test';

test('weekly quota stays enabled and per-agent bar choices are staged until saved', async ({
  page,
}) => {
  await page.goto('/');
  const settings = page.getByRole('button', {
    name: 'Einstellungen',
    exact: true,
  });
  await settings.click();
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  const group = dialog.getByRole('region', { name: 'Kontingentbalken' });
  const preview = group.getByRole('group', { name: 'Vorschau' });
  const bars = preview.locator('[data-metric]');
  const agent = dialog.getByRole('radiogroup', { name: 'Agent', exact: true });
  const week = group.getByRole('checkbox', { name: 'Woche', exact: true });
  const fable = group.getByRole('checkbox', { name: 'Fable', exact: true });
  const save = dialog.getByRole('button', { name: 'Speichern', exact: true });
  await expect(week).toBeChecked();
  await expect(week).toBeDisabled();
  await expect(bars).toHaveCount(1);
  await expect(bars.first()).toHaveAttribute('data-metric', 'week');
  await fable.check();
  await group.getByRole('button', { name: 'Woche nach unten' }).click();
  await expect(bars.first()).toHaveAttribute('data-metric', 'fable');
  await dialog.getByRole('button', { name: 'Abbrechen' }).click();

  await settings.click();
  await expect(fable).not.toBeChecked();
  await expect(bars).toHaveCount(1);
  await expect(save).toHaveAttribute('aria-disabled', 'true');
  await fable.check();
  await group.getByRole('button', { name: 'Fable nach oben' }).click();
  await agent.getByRole('radio', { name: 'OpenCode' }).check();
  await expect(fable).toHaveCount(0);
  await group.getByRole('checkbox', { name: 'Monat' }).check();
  await agent.getByRole('radio', { name: 'Codex' }).check();
  await expect(bars).toHaveCount(1);
  await group.getByRole('checkbox', { name: 'Spark-Woche' }).check();
  await save.click();
  await expect(dialog).toHaveCount(0);

  await settings.click();
  await expect(fable).toBeChecked();
  await expect(bars.nth(0)).toHaveAttribute('data-metric', 'fable');
  await expect(bars.nth(1)).toHaveAttribute('data-metric', 'week');
  await agent.getByRole('radio', { name: 'OpenCode' }).check();
  await expect(bars.nth(1)).toHaveAttribute('data-metric', 'month');
  await agent.getByRole('radio', { name: 'Codex' }).check();
  await expect(bars.nth(1)).toHaveAttribute('data-metric', 'sparkWeek');
});
