import { describe, expect, it } from 'vitest';
import {
  RECENT_LIMIT,
  editRecent,
  recentFolders,
  recentId,
  recordStart,
  restoreRecent,
  upsertRecent,
  withoutRecent,
} from './recents';
import type { LaunchRequest, RecentSession } from './types';

const options = {
  model: 'default',
  effort: 'default',
  permission: 'ask' as const,
  extraArgs: '',
};
function recent(folder: string, minutesAgo: number): RecentSession {
  return {
    id: recentId(folder, 'claude', 'local'),
    agent: 'claude',
    env: 'local',
    folder,
    options,
    ports: [],
    lastStartedAt: new Date(
      Date.parse('2026-11-03T12:00:00Z') - minutesAgo * 60_000,
    ).toISOString(),
    starts: 1,
  };
}
const list = [recent('C:\\a', 1), recent('C:\\b', 5), recent('C:\\c', 9)];
const ids = (items: RecentSession[]) => items.map((item) => item.folder);

describe('recents', () => {
  it('ids one entry per lowercased folder, agent and env', () => {
    expect(recentId('C:\\Repo', 'codex', 'docker')).toBe(
      recentId('c:\\repo ', 'codex', 'docker'),
    );
    expect(recentId('C:\\Repo', 'codex', 'docker')).not.toBe(
      recentId('C:\\Repo', 'codex', 'local'),
    );
  });

  it('records a start: new, or the existing entry bumped to the top', () => {
    const request: LaunchRequest = {
      agent: 'claude',
      env: 'local',
      folder: 'C:\\c',
      options: { ...options, permission: 'bypass' },
      session: 'continue',
      ports: [],
    };
    const now = new Date('2026-11-03T12:30:00Z');
    const entry = recordStart(list, request, now);
    expect(entry).toMatchObject({
      id: list[2]!.id,
      starts: 2,
      lastStartedAt: now.toISOString(),
      options: { permission: 'bypass' },
    });
    expect(ids(upsertRecent(list, entry))).toEqual(['C:\\c', 'C:\\a', 'C:\\b']);
    expect(
      recordStart(list, { ...request, folder: 'C:\\new' }, now).starts,
    ).toBe(1);
  });

  it(`keeps at most ${RECENT_LIMIT}`, () => {
    const many = Array.from({ length: RECENT_LIMIT }, (_, i) =>
      recent(`C:\\r${i}`, i + 1),
    );
    const next = upsertRecent(many, recent('C:\\newest', 0));
    expect(next).toHaveLength(RECENT_LIMIT);
    expect(next[0]?.folder).toBe('C:\\newest');
    expect(next.at(-1)?.folder).toBe(`C:\\r${RECENT_LIMIT - 2}`);
  });

  it('undoes a removal at the entry’s place by start time', () => {
    const removed = list[1]!;
    const without = withoutRecent(list, removed.id);
    expect(ids(without)).toEqual(['C:\\a', 'C:\\c']);
    expect(ids(restoreRecent(without, removed)!)).toEqual(ids(list));
    // A second restore never duplicates the entry.
    expect(ids(restoreRecent(list, removed)!)).toEqual(ids(list));
  });

  it('keeps an entry started again since the removal, like the native core', () => {
    const removed = list[1]!;
    const restarted: RecentSession = {
      ...removed,
      lastStartedAt: '2026-11-03T12:30:00.000Z',
      starts: 2,
    };
    const current = upsertRecent(withoutRecent(list, removed.id), restarted);
    expect(restoreRecent(current, removed)).toEqual(current);
    // The id is recomputed from folder, agent and env, never trusted.
    const forged = { ...removed, id: 'anything' };
    expect(restoreRecent(current, forged)).toEqual(current);
    expect(
      restoreRecent(withoutRecent(list, removed.id), forged)?.map((r) => r.id),
    ).toEqual(list.map((r) => r.id));
  });

  it('refuses an entry without a readable time or start count', () => {
    expect(
      restoreRecent([], { ...list[0]!, lastStartedAt: 'yesterday' }),
    ).toBeNull();
    expect(restoreRecent([], { ...list[0]!, starts: 0 })).toBeNull();
  });

  it('lists distinct folders newest first', () => {
    const docker = { ...list[0]!, id: 'other', env: 'docker' as const };
    expect(recentFolders([list[0]!, docker, ...list.slice(1)])).toEqual([
      'C:\\a',
      'C:\\b',
      'C:\\c',
    ]);
    expect(recentFolders(list, 2)).toEqual(['C:\\a', 'C:\\b']);
  });

  it('edits options and ports in place and keeps time and count', () => {
    const list = [recent('C:\\a', 1), recent('C:\\b', 2)];
    const next = { ...options, model: ' opus ', permission: 'bypass' as const };
    const edited = editRecent(list, list[1]!.id, next, ['127.0.0.1:80:80'])!;
    expect(edited.map((r) => r.id)).toEqual(list.map((r) => r.id));
    expect(edited[1]).toEqual({
      ...list[1],
      options: { ...next, model: 'opus' },
      ports: ['127.0.0.1:80:80'],
    });
    expect(edited[0]).toBe(list[0]);
    expect(editRecent(list, 'unknown', next, [])).toBeNull();
  });
});
