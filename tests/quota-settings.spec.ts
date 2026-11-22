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
  const group = dialog.getByRole('region', { name: 'Kontingent', exact: true });
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
  await expect(bars).toHaveCount(0);
  const sparkWeek = group.getByRole('checkbox', { name: 'Spark-Woche' });
  await expect(sparkWeek).toBeDisabled();
  // The switch input is visually hidden; its label toggles it.
  await group.getByText('Kontingent abfragen', { exact: true }).click();
  await expect(bars).toHaveCount(1);
  await sparkWeek.check();
  await save.click();
  await expect(dialog).toHaveCount(0);

  await settings.click();
  await expect(fable).toBeChecked();
  await expect(bars.nth(0)).toHaveAttribute('data-metric', 'fable');
  await expect(bars.nth(1)).toHaveAttribute('data-metric', 'week');
  await agent.getByRole('radio', { name: 'OpenCode' }).check();
  await expect(bars.nth(1)).toHaveAttribute('data-metric', 'month');
  await agent.getByRole('radio', { name: 'Codex' }).check();
  await expect(
    group.getByRole('checkbox', { name: 'Kontingent abfragen' }),
  ).toBeChecked();
  await expect(bars.nth(1)).toHaveAttribute('data-metric', 'sparkWeek');
});

test('the quota read switch is staged until saved and locks the bar choices while off', async ({
  page,
}) => {
  await page.goto('/');
  const settings = page.getByRole('button', {
    name: 'Einstellungen',
    exact: true,
  });
  await settings.click();
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  const group = dialog.getByRole('region', { name: 'Kontingent', exact: true });
  const read = group.getByRole('checkbox', { name: 'Kontingent abfragen' });
  const flip = () =>
    group.getByText('Kontingent abfragen', { exact: true }).click();
  const fable = group.getByRole('checkbox', { name: 'Fable', exact: true });
  const preview = group.getByRole('group', { name: 'Vorschau' });
  const save = dialog.getByRole('button', { name: 'Speichern', exact: true });
  const claude = page
    .getByRole('radiogroup', { name: 'Agent' })
    .first()
    .locator('label:has(input[value="claude"])');
  await expect(group.getByText('Weitere Balken')).toHaveCount(0);
  await expect(group.locator('label.switch-row')).toHaveAttribute(
    'title',
    /CLI-Anmeldung.*nicht offiziell dokumentierte Schnittstelle/,
  );
  await expect(read).toBeChecked();
  await expect(fable).toBeEnabled();
  await fable.check();
  const up = group.getByRole('button', { name: 'Fable nach oben' });
  await expect(up).toBeEnabled();
  await flip();
  await expect(fable).toBeDisabled();
  await expect(fable).toBeChecked();
  await expect(up).toBeDisabled();
  await expect(preview.locator('[data-metric], .weekly-summary')).toHaveCount(
    0,
  );
  await expect(save).not.toHaveAttribute('aria-disabled', 'true');
  await dialog.getByRole('button', { name: 'Abbrechen' }).click();

  await settings.click();
  await expect(read).toBeChecked();
  await flip();
  await save.click();
  await expect(dialog).toHaveCount(0);
  await expect(claude.locator('.quota-meter, .weekly-summary')).toHaveCount(0);
  await settings.click();
  await expect(read).not.toBeChecked();
  await expect(fable).toBeDisabled();
});
