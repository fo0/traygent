/*
 * Synthetic GitHub data of the browser preview: an invented account, its
 * invented repositories and gh sign-ins. Nothing is read from GitHub or the
 * machine; every login and repository is labelled `demo-…`.
 */
import type {
  GithubAccount,
  GithubState,
  LocalAccounts,
  RepoSummary,
} from './types';

export const DEMO_CLONE_ROOT = 'C:\\Users\\demo\\Traygent';
export const DEMO_LOGIN = 'demo-user';

export const DEMO_ACCOUNT: GithubAccount = {
  login: DEMO_LOGIN,
  id: 1_000_001,
  source: 'token',
};

const HOUR = 60 * 60_000;

/** `[owner, name, private, hours since the last push, description]`, newest first. */
const REPOS: [string, string, boolean, number, string?][] = [
  [DEMO_LOGIN, 'traygent', false, 1, 'Tray-App für Coding-Agent-Sessions'],
  [DEMO_LOGIN, 'repo-watch', false, 5, 'GitHub-Actions im Tray'],
  ['demo-org', 'kunden-portal', true, 20, 'Portal für Kunden (Demo)'],
  ['demo-org', 'infra-terraform', true, 30],
  [DEMO_LOGIN, 'agent-tools', false, 50, 'Docker-Rezepte für Agenten'],
  ['demo-org', 'design-system', false, 72, 'Gemeinsame UI-Bausteine'],
  [DEMO_LOGIN, 'notizen', true, 96],
  ['demo-org', 'api-gateway', true, 140, 'Gateway der Demo-Dienste'],
  [DEMO_LOGIN, 'dotfiles', false, 200],
  ['demo-org', 'mobile-app', true, 320, 'App für iOS und Android'],
  [DEMO_LOGIN, 'tray-experiments', false, 500],
  ['demo-org', 'docs', false, 900, 'Handbuch der Demo-Organisation'],
];

/** The demo account's listing relative to `now`, newest push first. */
export function demoRepos(now: number = Date.now()): RepoSummary[] {
  return REPOS.map(([owner, name, isPrivate, hours, description]) => ({
    owner,
    name,
    private: isPrivate,
    pushedAt: new Date(now - hours * HOUR).toISOString(),
    ...(description && { description }),
  }));
}

/** Connected with a token, listed at `now`. */
export function demoGithub(now: number = Date.now()): GithubState {
  return {
    account: { ...DEMO_ACCOUNT },
    repos: demoRepos(now),
    loading: false,
    listedAt: new Date(now).toISOString(),
    defaultCloneRoot: DEMO_CLONE_ROOT,
  };
}

/** The gh sign-ins the demo „finds“: one importable, one that is not. */
export function demoLocalAccounts(connected?: string): LocalAccounts {
  return {
    accounts: [
      {
        login: DEMO_LOGIN,
        importable: true,
        connected: connected === DEMO_LOGIN,
      },
      { login: 'demo-ci', importable: false, connected: false },
    ],
  };
}
