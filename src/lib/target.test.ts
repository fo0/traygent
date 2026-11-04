import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  LISTING_MAX_AGE,
  classifyTarget,
  listingStale,
  matchRepos,
  repoLabel,
  targetProblem,
  type Target,
} from './target';
import type { GithubState, RepoSummary } from './types';

type Case = {
  input: string;
  kind: Target['kind'];
  owner?: string;
  name?: string;
  query?: string;
};
const { cases }: { cases: Case[] } = JSON.parse(
  readFileSync(join(process.cwd(), 'fixtures/targets.json'), 'utf8'),
);

/** The fixture's flat form of a classification. */
function flat(target: Target): Omit<Case, 'input'> {
  switch (target.kind) {
    case 'repo':
      return { kind: 'repo', ...target.repo };
    case 'search':
      return { kind: 'search', query: target.query };
    default:
      return { kind: target.kind };
  }
}

describe('classifyTarget', () => {
  it('has shared test vectors', () => expect(cases.length).toBeGreaterThan(20));

  it.each(cases.map((entry) => [JSON.stringify(entry.input), entry] as const))(
    'classifies %s as in fixtures/targets.json',
    (_, { input, ...expected }) => {
      expect(flat(classifyTarget(input))).toEqual(expected);
    },
  );

  it('keeps the trimmed folder path', () => {
    expect(classifyTarget('  C:\\dev\\app ')).toEqual({
      kind: 'folder',
      folder: 'C:\\dev\\app',
    });
  });

  it('checks owner and name lengths', () => {
    expect(classifyTarget(`${'a'.repeat(39)}/x`).kind).toBe('repo');
    expect(classifyTarget(`${'a'.repeat(40)}/x`).kind).toBe('search');
    expect(classifyTarget(`o/${'n'.repeat(100)}`).kind).toBe('repo');
    expect(classifyTarget(`o/${'n'.repeat(101)}`).kind).toBe('search');
    expect(classifyTarget('o/.git').kind).toBe('search');
  });

  it('names what a start still needs', () => {
    expect(targetProblem(classifyTarget(''))).toBe('Ordner wählen');
    expect(targetProblem(classifyTarget('tray'))).toBe(
      'Ordner oder Repo wählen',
    );
    expect(targetProblem(classifyTarget('fo0/traygent'))).toBe('');
    expect(targetProblem(classifyTarget('C:\\x'))).toBe('');
    expect(repoLabel({ owner: 'fo0', name: 'traygent' })).toBe('fo0/traygent');
  });
});

const repo = (owner: string, name: string): RepoSummary => ({
  owner,
  name,
  private: false,
});

describe('matchRepos', () => {
  const repos = [
    repo('demo-user', 'traygent'),
    repo('demo-org', 'Tray-Tools'),
    repo('demo-user', 'repo-watch'),
    repo('other', 'agent-tray'),
  ];

  it('needs every word in owner/name, case-insensitive, in listing order', () => {
    expect(matchRepos(repos, 'TRAY').map(repoLabel)).toEqual([
      'demo-user/traygent',
      'demo-org/Tray-Tools',
      'other/agent-tray',
    ]);
    expect(matchRepos(repos, 'tray  demo').map(repoLabel)).toEqual([
      'demo-user/traygent',
      'demo-org/Tray-Tools',
    ]);
    expect(matchRepos(repos, 'user/repo').map(repoLabel)).toEqual([
      'demo-user/repo-watch',
    ]);
    expect(matchRepos(repos, 'nothing')).toEqual([]);
  });

  it('keeps at most the limit', () => {
    const many = Array.from({ length: 20 }, (_, i) => repo('o', `r${i}`));
    expect(matchRepos(many, 'r')).toHaveLength(8);
    expect(matchRepos(many, 'r', 3).map((r) => r.name)).toEqual([
      'r0',
      'r1',
      'r2',
    ]);
  });
});

describe('listingStale', () => {
  const now = Date.parse('2026-11-04T12:00:00Z');
  const state = (patch: Partial<GithubState>, connected = true) => {
    const value: GithubState = {
      repos: [],
      loading: false,
      defaultCloneRoot: 'C:\\Users\\demo\\Traygent',
      ...patch,
    };
    if (connected)
      value.account = { login: 'demo-user', id: 1, source: 'token' };
    return value;
  };
  const ago = (ms: number) => new Date(now - ms).toISOString();

  it('is stale after 15 minutes or without a listing', () => {
    expect(listingStale(state({ listedAt: ago(60_000) }), now)).toBe(false);
    expect(
      listingStale(state({ listedAt: ago(LISTING_MAX_AGE + 1) }), now),
    ).toBe(true);
    expect(listingStale(state({}), now)).toBe(true);
  });

  it('never while loading or without an account', () => {
    expect(listingStale(state({ loading: true }), now)).toBe(false);
    expect(listingStale(state({}, false), now)).toBe(false);
  });
});
