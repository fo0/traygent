import { describe, expect, it, vi } from 'vitest';
import { createDemoBridge } from './demo';
import { GITHUB_NOTICE } from './githubActions.svelte';
import { Panel } from './panel.svelte';
import { LISTING_MAX_AGE } from './target';
import type { Snapshot } from './types';

const idle = async () => () => {};

async function loadedPanel() {
  const bridge = createDemoBridge(0, 0);
  const panel = new Panel({ bridge, native: false, observe: bridge.observe });
  panel.load();
  await vi.waitFor(() => expect(panel.snapshot).not.toBeNull());
  return { panel, bridge };
}

describe('GitHub actions', () => {
  it('connects with a notice and disconnects without one', async () => {
    const { panel } = await loadedPanel();
    await panel.github.disconnect();
    expect(panel.snapshot?.github.account).toBeUndefined();
    await expect(panel.github.connectToken('nope')).rejects.toThrow(
      'GitHub hat das Token abgelehnt.',
    );
    await panel.github.connectToken('ghp_demo1234');
    expect(panel.snapshot?.github.account?.login).toBe('demo-user');
    expect(panel.notices.get(GITHUB_NOTICE)).toMatchObject({
      text: 'Konto demo-user verbunden.',
      tone: 'subtle',
      timeout: 3000,
    });
    await panel.github.disconnect();
    expect(panel.notices.get(GITHUB_NOTICE)).toBeUndefined();
    await panel.github.importLocal('demo-user');
    expect(panel.snapshot?.github.account?.source).toBe('gh');
  });

  it('refreshes a stale listing on focus, once per 15 minutes', async () => {
    const listedAt = '2026-11-04T10:00:00Z';
    const base = createDemoBridge(0, 0);
    const first = await base.snapshot();
    const stale: Snapshot = { ...first, github: { ...first.github, listedAt } };
    const refresh = vi.fn(async () => {
      throw 'GitHub-Token ungültig oder abgelaufen.';
    });
    const panel = new Panel({
      bridge: {
        ...base,
        snapshot: async () => stale,
        githubRefreshRepos: refresh,
      },
      native: true,
      observe: idle,
    });
    panel.load();
    await vi.waitFor(() => expect(panel.snapshot).not.toBeNull());
    const now = Date.parse(listedAt) + LISTING_MAX_AGE + 1;
    panel.github.refreshIfStale(Date.parse(listedAt) + 60_000);
    expect(refresh).not.toHaveBeenCalled();
    panel.github.refreshIfStale(now);
    panel.github.refreshIfStale(now);
    await vi.waitFor(() =>
      expect(panel.notices.get(GITHUB_NOTICE)?.text).toBe(
        'GitHub-Token ungültig oder abgelaufen.',
      ),
    );
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(panel.notices.get(GITHUB_NOTICE)?.tone).toBe('warning');
    // A failure does not retry on every focus.
    panel.github.refreshIfStale(now + 60_000);
    expect(refresh).toHaveBeenCalledTimes(1);
    panel.github.refreshIfStale(now + LISTING_MAX_AGE);
    expect(refresh).toHaveBeenCalledTimes(2);
  });

  it('never refreshes without an account', async () => {
    const { panel } = await loadedPanel();
    await panel.github.disconnect();
    const spy = vi.spyOn(panel.github, 'refresh');
    panel.github.refreshIfStale(Date.now() + 10 * LISTING_MAX_AGE);
    expect(spy).not.toHaveBeenCalled();
  });
});
