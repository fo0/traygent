import type { AgentQuota } from '../src/lib/quotaTypes';
import type { AgentId } from '../src/lib/types';
import { pinFont } from './support/fonts';
import { nativeSnapshot } from './support/snapshot';
import { installTauriMock } from './support/tauriMock';
import { expect, test, type Page } from './support/test';

const clock = '2026-11-22T12:00:00Z';
const resetAt = '2026-11-25T12:00:00Z';
const group = (page: Page) => page.getByRole('radiogroup', { name: 'Agent' });
const segment = (page: Page, agent: AgentId) =>
  group(page).locator(`label:has(input[value="${agent}"])`);

/** Fixed provider readings via the labelled IPC mock; the composer uses its real cache. */
async function serveQuotas(
  page: Page,
  quotas: Partial<Record<AgentId, AgentQuota>>,
  reads: Record<AgentId, boolean> = {
    claude: true,
    opencode: true,
    codex: true,
  },
) {
  const snapshot = nativeSnapshot();
  snapshot.settings.quotaBars = {
    claude: ['session', 'week', 'fable'],
    codex: ['week', 'spark'],
  };
  snapshot.settings.quotaReads = reads;
  snapshot.settings.quotaRefreshMinutes = 12;
  await installTauriMock(
    page,
    ({ snapshot, quotas }) =>
      (cmd, args) => {
        if (cmd === 'snapshot') return snapshot;
        if (cmd === 'get_agent_quota') return quotas[args?.agent as AgentId];
        return null;
      },
    { arg: { snapshot, quotas }, state: {} },
  );
  await page.clock.install({ time: new Date(clock) });
  await page.goto('/');
}

test('the selected CLI shows its ordered bars, while the other radios expose weekly usage only', async ({
  page,
}) => {
  await serveQuotas(page, {
    claude: {
      windows: {
        week: { used: 32, resetAt },
        session: { used: 70, resetAt },
        fable: { used: 90, resetAt },
      },
      detail: null,
    },
    codex: {
      windows: {
        week: { used: 100, resetAt: null },
        spark: { used: 0, resetAt: null },
      },
      detail: null,
    },
    opencode: { windows: {}, detail: 'Anmeldung erforderlich.' },
  });
  const claude = segment(page, 'claude');
  const codex = segment(page, 'codex');
  const openCode = segment(page, 'opencode');
  const localTime = await page.evaluate(
    (time) =>
      new Date(time).toLocaleTimeString('de-DE', {
        hour: '2-digit',
        minute: '2-digit',
      }),
    clock,
  );
  const updated = claude.locator('.quota-updated');
  await expect(updated).toHaveText(`Stand ${localTime}`);
  await updated.hover();
  await expect(updated).toHaveAttribute(
    'title',
    /Zuletzt aktualisiert am .*alle 12 Min/,
  );
  // The used share in the configured order; the weekly reset on the right.
  await expect(claude.locator('.quota-metric')).toHaveText([
    'Session 70 %',
    'Woche 32 %',
    'Fable 90 %',
  ]);
  await expect(claude.locator('.quota-name')).toHaveText('Standard:');
  await expect(claude.locator('.quota-countdown')).toHaveText('3 Tage');
  for (const inactive of [codex, openCode]) {
    await expect(inactive.locator('.quota-updated, .quota-line')).toHaveCount(
      0,
    );
    await expect(inactive.locator('.quota-meter')).toHaveCount(1);
  }
  // Each metric keeps its colour; the figure and the bar's end show the state.
  for (const [metric, token, used, cap] of [
    ['session', '--wait', 70, true],
    ['week', '--quota-week', 32, false],
    ['fable', '--fail', 90, true],
  ] as const) {
    const figure = claude.locator(
      `.quota-metric[data-metric="${metric}"] .quota-figure`,
    );
    const fill = claude.locator(
      `.quota-track[data-metric="${metric}"] .quota-fill`,
    );
    const colour = await figure.evaluate((node, token) => {
      // Resolve the token in the active theme through the same CSS colour property.
      const probe = document.createElement('span');
      probe.style.color = `var(${token})`;
      node.append(probe);
      const expected = getComputedStyle(probe).color;
      probe.remove();
      return { expected, actual: getComputedStyle(node).color };
    }, token);
    expect(colour.actual, metric).toBe(colour.expected);
    const bar = await fill.evaluate((node) => ({
      image: getComputedStyle(node).backgroundImage,
      width: (node as HTMLElement).style.width,
    }));
    expect(bar.width).toBe(`${used}%`);
    expect(bar.image.startsWith('linear-gradient'), metric).toBe(cap);
  }
  const claudeRadio = claude.getByRole('radio', {
    name: 'Claude Code',
    exact: true,
  });
  await expect(claudeRadio).toHaveAccessibleDescription(
    /Session · 70 % verbraucht · 30 % frei.*Woche · 32 % verbraucht · 68 % frei.*Fable · 90 % verbraucht/,
  );
  await expect(claude).toHaveAttribute('title', /Reset am 25\.11\.2026/);
  await claudeRadio.press('ArrowRight');
  await expect(openCode.getByRole('radio')).toBeChecked();
  await expect(openCode.getByRole('radio')).toHaveAccessibleDescription(
    /Keine Daten.*Anmeldung erforderlich/,
  );
  await expect(openCode.locator('.quota-updated')).toHaveText('Stand –');
  await expect(openCode.locator('.quota-fill')).toHaveCount(0);
  await expect(claude.locator('.quota-meter')).toHaveCount(1);
  await openCode.getByRole('radio').press('ArrowRight');
  await expect(codex.getByRole('radio')).toBeFocused();
  await expect(codex.locator('.quota-metric')).toHaveText([
    'Woche 100 %',
    'Spark 0 %',
  ]);
  await expect(codex.locator('.quota-name, .quota-countdown')).toHaveCount(0);
});

test('an agent whose read is off is never asked and shows no figure or bar', async ({
  page,
}) => {
  const reading: AgentQuota = {
    windows: { week: { used: 32, resetAt } },
    detail: null,
  };
  await serveQuotas(
    page,
    { claude: reading, opencode: reading, codex: reading },
    { claude: false, opencode: true, codex: false },
  );
  const claude = segment(page, 'claude');
  await expect(segment(page, 'opencode').locator('.quota-fill')).toHaveCount(1);
  await expect(claude.locator('.quota-meter, .quota-updated')).toHaveCount(0);
  await expect(claude.locator('.segment-label')).toHaveText('Claude Code');
  await expect(
    claude.getByRole('radio', { name: 'Claude Code', exact: true }),
  ).toHaveAccessibleDescription(/Kontingentabfrage ausgeschaltet/);
  await expect(segment(page, 'codex').locator('.quota-meter')).toHaveCount(0);
  const asked = await page.evaluate(() =>
    window
      .__traygentIpcMock!.calls.filter((c) => c.cmd === 'get_agent_quota')
      .map((c) => c.args?.agent),
  );
  expect([...new Set(asked)]).toEqual(['opencode']);
});

test('expired readings disappear while a still-current window stays visible', async ({
  page,
}) => {
  await serveQuotas(page, {
    claude: {
      windows: {
        week: { used: 92, resetAt: '2026-11-22T12:00:13Z' },
        session: { used: 30, resetAt },
      },
      detail: null,
    },
  });
  const selected = segment(page, 'claude');
  const week = selected.locator('.quota-metric[data-metric=week]');
  await expect(week).toHaveText('Woche 92 %');
  await page.clock.fastForward(13_100);
  await expect(week).toHaveText('Woche –');
  await expect(week).toHaveAttribute('data-level', 'unknown');
  await expect(
    selected.locator('.quota-track[data-metric=week].unknown'),
  ).toHaveCount(1);
  await expect(selected.getByRole('radio')).toHaveAccessibleDescription(
    /Woche · Zeitfenster abgelaufen/,
  );
  await expect(
    selected.locator(
      '[data-metric="week"] .quota-fill, [data-metric="fable"] .quota-fill',
    ),
  ).toHaveCount(0);
  await expect(selected.locator('.quota-metric')).toHaveText([
    'Session 30 %',
    'Woche –',
    'Fable –',
  ]);
});

test('a long countdown and fractional reading fit at narrow widths', async ({
  page,
}) => {
  await serveQuotas(page, {
    claude: {
      windows: { week: { used: 0.3, resetAt: '2026-11-22T12:59:00Z' } },
      detail: null,
    },
  });
  await pinFont(page, 'DejaVu Sans');
  const selected = segment(page, 'claude');
  await expect(selected.locator('.quota-metric')).toHaveText([
    'Session –',
    'Woche 0,3 %',
    'Fable –',
  ]);
  await expect(selected.locator('.quota-countdown')).toHaveText('59 Min.');
  for (const width of [320, 351, 375, 398]) {
    await page.setViewportSize({ width, height: 760 });
    const { cut, figures, overflow, lines } = await selected.evaluate(
      (node) => {
        const name = node.querySelector('.segment-label')!;
        const metrics = node.querySelector('.quota-metrics')!;
        const summary = node
          .querySelector('.quota-countdown')!
          .getBoundingClientRect();
        const track = node
          .querySelector('.quota-track')!
          .getBoundingClientRect();
        return {
          cut: name.scrollWidth > name.clientWidth,
          figures: metrics.scrollWidth > metrics.clientWidth,
          overflow: Math.abs(track.right - summary.right),
          // One 11px line: nothing wraps.
          lines: Math.round(
            node.querySelector('.quota-line')!.getBoundingClientRect().height /
              13,
          ),
        };
      },
    );
    expect(cut, `${width}px`).toBe(false);
    expect(figures, `${width}px`).toBe(false);
    expect(overflow, `${width}px`).toBeLessThan(1);
    expect(lines, `${width}px`).toBe(1);
  }
});
