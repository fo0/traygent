import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DownloadProgress, UpdateInfo } from './types';
import { Updater } from './updater.svelte';
import { changelogSummary, releaseDate } from './updates';

// Labelled IPC mock: the controller reaches the shell only through ipc.ts.
const ipc = vi.hoisted(() => ({
  call: vi.fn<(command: string, args?: unknown) => Promise<unknown>>(),
}));
vi.mock('./ipc', () => ({
  call: ipc.call,
  channel: <T>(onMessage: (message: T) => void) => ({ onMessage }),
}));

// Synthetic release notes in the shape of GitHub's generated changelog.
const GENERATED = `Release 2026.1101.905

Version: 2026.1101.905
Source: 0123456789abcdef0123456789abcdef01234567

## What's Changed
* feat: install updates in the app by @synthetic-user in https://github.com/fo0/traygent/pull/18
* fix(panel): keep the **banner** compact by @dependabot[bot] in https://github.com/fo0/traygent/pull/19
- docs: explain \`install_update\` and [releases](https://example.invalid/x) by @a.b in https://github.com/fo0/traygent/pull/20

## New Contributors
* @synthetic-user made their first contribution in https://github.com/fo0/traygent/pull/18

**Full Changelog**: https://github.com/fo0/traygent/compare/v1...v2

Windows installer: not Authenticode signed.`;

describe('changelogSummary', () => {
  it('keeps list items without author, pull request link or markdown', () => {
    expect(changelogSummary(GENERATED)).toEqual([
      'feat: install updates in the app',
      'fix(panel): keep the banner compact',
      'docs: explain install_update and releases',
    ]);
  });

  it('caps the summary at five short bullets', () => {
    const notes = Array.from(
      { length: 8 },
      (_, index) => `* change ${index} ${'x'.repeat(index === 0 ? 200 : 0)}`,
    ).join('\n');
    const bullets = changelogSummary(notes);
    expect(bullets).toHaveLength(5);
    expect(bullets[0]).toHaveLength(120);
    expect(bullets[0]?.endsWith('…')).toBe(true);
    expect(bullets.at(-1)).toBe('change 4');
  });

  it('skips headings, Full Changelog lines, empty and plain lines', () => {
    expect(
      changelogSummary(
        '# Heading\n\n*   \n- \nPlain text\n* * Full Changelog: x\r\n* kept\r\n',
      ),
    ).toEqual(['kept']);
    expect(changelogSummary('')).toEqual([]);
    expect(
      changelogSummary('No additional release notes were generated.'),
    ).toEqual([]);
  });

  it('formats release dates in German and tolerates bad input', () => {
    expect(releaseDate('2026-11-01T09:05:00Z')).toBe('01.11.2026');
    expect(releaseDate('not a date')).toBe('');
    expect(releaseDate(null)).toBe('');
  });
});

const INFO: UpdateInfo = {
  version: '2026.1101.905',
  notes: '* synthetic change',
  date: null,
};

describe('Updater', () => {
  beforeEach(() => ipc.call.mockReset());

  it('checks, then installs in one call while following the stages', async () => {
    const updater = new Updater(true);
    ipc.call.mockResolvedValueOnce(INFO);
    await updater.check();
    expect(updater.phase).toBe('available');
    expect(updater.info).toEqual(INFO);

    const seen: string[] = [];
    ipc.call.mockImplementationOnce(async (_command, args) => {
      const { onProgress } = args as {
        onProgress: { onMessage: (value: DownloadProgress) => void };
      };
      onProgress.onMessage({ downloaded: 5, total: 10, stage: 'downloading' });
      seen.push(`${updater.phase} ${updater.progress.downloaded}`);
      onProgress.onMessage({ downloaded: 0, total: null, stage: 'verifying' });
      seen.push(updater.phase);
      // A stale download message never moves the stage back.
      onProgress.onMessage({ downloaded: 6, total: 10, stage: 'downloading' });
      seen.push(updater.phase);
      onProgress.onMessage({ downloaded: 0, total: null, stage: 'installing' });
      seen.push(updater.phase);
    });
    await updater.install();
    expect(ipc.call.mock.calls.map(([command]) => command)).toEqual([
      'check_update',
      'install_update',
    ]);
    expect(seen).toEqual([
      'downloading 5',
      'verifying',
      'verifying',
      'installing',
    ]);
    expect(updater.phase).toBe('installing');
    expect(updater.busy).toBe(true);
  });

  it('returns to available with the error, so the same button retries', async () => {
    const updater = new Updater(true);
    ipc.call.mockResolvedValueOnce(INFO);
    await updater.check();
    ipc.call.mockRejectedValueOnce('Prüfung fehlgeschlagen.');
    await updater.install();
    expect(updater.phase).toBe('available');
    expect(updater.message).toBe('Prüfung fehlgeschlagen.');
    ipc.call.mockResolvedValueOnce(undefined);
    await updater.install();
    expect(updater.message).toBe('');
    expect(updater.phase).toBe('installing');
  });

  it('reports a current version, hides silent failures and never runs in the demo', async () => {
    const updater = new Updater(true);
    ipc.call.mockResolvedValueOnce(null);
    await updater.check();
    expect(updater.phase).toBe('current');
    ipc.call.mockRejectedValueOnce('Offline.');
    await updater.check(true);
    expect([updater.phase, updater.message, updater.info]).toEqual([
      'idle',
      '',
      null,
    ]);
    await updater.install();
    expect(ipc.call).toHaveBeenCalledTimes(2);

    const demo = new Updater(false);
    await demo.check();
    await demo.install();
    expect(ipc.call).toHaveBeenCalledTimes(2);
  });
});
