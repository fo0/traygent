import type { AgentQuota } from '../src/lib/quotaTypes';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock } from './support/tauriMock';
import { expect, test } from './support/test';

/*
 * The selected agent's tile with several profiles (#94): one label line per
 * shown profile with its name, the used share per metric and the weekly
 * countdown, each read with its own profile. Labelled native-IPC mock,
 * synthetic data only.
 */
const clock = '2026-11-22T12:00:00Z';

test('two shown profiles get a label line each, with names, used shares and the countdown', async ({
  page,
}) => {
  const snapshot = nativeSnapshot();
  snapshot.settings.quotaReads = {
    claude: true,
    opencode: false,
    codex: false,
  };
  snapshot.settings.quotaBars = { claude: ['week', 'fable'] };
  snapshot.settings.quotaProfiles = { claude: ['team'] };
  const quotas: Record<string, AgentQuota> = {
    default: {
      windows: {
        week: { used: 64, resetAt: '2026-11-25T12:00:00Z' },
        fable: { used: 81, resetAt: '2026-11-25T12:00:00Z' },
      },
      detail: null,
    },
    team: {
      windows: {
        week: { used: 93, resetAt: '2026-11-22T17:00:00Z' },
        fable: { used: 12.5, resetAt: null },
      },
      detail: null,
    },
  };
  await installTauriMock(
    page,
    ({ snapshot, quotas }) =>
      (cmd, args) => {
        if (cmd === 'snapshot') return snapshot;
        if (cmd === 'get_agent_quota')
          return quotas[args?.profile as string] ?? null;
        return null;
      },
    { arg: { snapshot, quotas }, state: {} },
  );
  await page.clock.install({ time: new Date(clock) });
  await page.goto('/');

  const claude = page
    .getByRole('radiogroup', { name: 'Agent' })
    .locator('label:has(input[value="claude"])');
  const blocks = claude.locator('.quota-profile');
  await expect(blocks).toHaveCount(2);
  await expect(blocks.locator('.quota-name')).toHaveText([
    'Standard:',
    'Team:',
  ]);
  await expect(blocks.nth(0).locator('.quota-metric')).toHaveText([
    'Woche 64 %',
    'Fable 81 %',
  ]);
  // Reads of one provider run one after another, 5–7 s apart (#100).
  await page.clock.fastForward(7_000);
  await expect(blocks.nth(1).locator('.quota-metric')).toHaveText([
    'Woche 93 %',
    'Fable 12,5 %',
  ]);
  await expect(blocks.locator('.quota-countdown')).toHaveText([
    '3 Tage',
    '5 Std.',
  ]);
  // The start profile's name stands out; the bars follow each line's figures.
  await expect(blocks.nth(0)).toHaveClass(/start/);
  await expect(blocks.nth(1)).not.toHaveClass(/start/);
  await expect(blocks.locator('.quota-fill')).toHaveCount(4);
  const widths = await blocks
    .locator('.quota-fill')
    .evaluateAll((nodes) =>
      nodes.map((node) => (node as HTMLElement).style.width),
    );
  expect(widths).toEqual(['64%', '81%', '93%', '12.5%']);
  await expect(
    blocks.nth(1).locator('.quota-metric[data-metric=week]'),
  ).toHaveAttribute('data-level', 'critical');
  await expect(
    claude.getByRole('radio', { name: 'Claude Code', exact: true }),
  ).toHaveAccessibleDescription(
    /Standard · Woche · 64 % verbraucht.*Team · Woche · 93 % verbraucht · 7 % frei/,
  );

  const asked = await page.evaluate(() =>
    window
      .__traygentIpcMock!.calls.filter((c) => c.cmd === 'get_agent_quota')
      .map((c) => `${c.args?.agent}/${c.args?.profile}`),
  );
  expect([...new Set(asked)].sort()).toEqual(['claude/default', 'claude/team']);
});
