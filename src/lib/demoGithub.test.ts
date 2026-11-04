import { describe, expect, it, vi } from 'vitest';
import { createDemoBridge } from './demo';
import { DEMO_CLONE_ROOT, demoRepos } from './demoGithub';
import type { LaunchRequest, Snapshot } from './types';

const request = (value: Partial<LaunchRequest> = {}): LaunchRequest => ({
  agent: 'claude',
  env: 'local',
  folder: '',
  repo: { owner: 'demo-org', name: 'kunden-portal' },
  options: {
    model: 'default',
    effort: 'default',
    permission: 'ask',
    extraArgs: '',
  },
  session: 'new',
  ports: [],
  ...value,
});

describe('demo GitHub data', () => {
  it('is a connected demo account with synthetic repositories, newest push first', async () => {
    const { github, settings } = await createDemoBridge(0).snapshot();
    expect(github.account).toEqual({
      login: 'demo-user',
      id: 1_000_001,
      source: 'token',
    });
    expect(github.repos.length).toBeGreaterThanOrEqual(10);
    expect(
      github.repos.every((repo) => /^demo-(?:user|org)$/.test(repo.owner)),
    ).toBe(true);
    const pushed = github.repos.map((repo) => Date.parse(repo.pushedAt!));
    expect(pushed).toEqual([...pushed].sort((a, b) => b - a));
    expect(github.repos.some((repo) => repo.private)).toBe(true);
    expect(github.defaultCloneRoot).toBe(DEMO_CLONE_ROOT);
    expect(github.loading).toBe(false);
    expect(settings.github.cloneRoot).toBe('');
    expect(demoRepos(0)[0]?.pushedAt).toBe(new Date(-3_600_000).toISOString());
  });
});

describe('demo GitHub commands', () => {
  it('disconnects, then connects by token and by the gh sign-in', async () => {
    const bridge = createDemoBridge(0);
    const off = await bridge.githubDisconnect();
    expect(off.github).toEqual({
      repos: [],
      loading: false,
      defaultCloneRoot: DEMO_CLONE_ROOT,
    });
    await expect(bridge.githubConnectToken(' ')).rejects.toThrow(
      'Token fehlt.',
    );
    await expect(bridge.githubConnectToken('falsch')).rejects.toThrow(
      'GitHub hat das Token abgelehnt.',
    );
    const token = await bridge.githubConnectToken('ghp_demo1234');
    expect(token.github.account?.source).toBe('token');
    expect(token.github.repos.length).toBeGreaterThan(0);
    expect(Date.parse(token.github.listedAt!)).not.toBeNaN();

    await bridge.githubDisconnect();
    const local = await bridge.githubLocalAccounts();
    expect(local.accounts).toEqual([
      { login: 'demo-user', importable: true, connected: false },
      { login: 'demo-ci', importable: false, connected: false },
    ]);
    await expect(bridge.githubImportLocal('demo-ci')).rejects.toThrow(
      'Die Anmeldung demo-ci lässt sich nicht übernehmen.',
    );
    const imported = await bridge.githubImportLocal('demo-user');
    expect(imported.github.account?.source).toBe('gh');
    expect((await bridge.githubLocalAccounts()).accounts[0]?.connected).toBe(
      true,
    );
  });

  it('answers a refresh with loading and pushes the finished listing', async () => {
    vi.useFakeTimers();
    try {
      const bridge = createDemoBridge(0, 300);
      const pushed: Snapshot[] = [];
      await bridge.observe((snapshot) => pushed.push(snapshot));
      const started = bridge.githubRefreshRepos();
      await vi.advanceTimersByTimeAsync(0);
      const answer = await started;
      expect(answer.github.loading).toBe(true);
      await vi.advanceTimersByTimeAsync(300);
      expect(pushed).toHaveLength(1);
      expect(pushed[0]?.github.loading).toBe(false);
      expect(pushed[0]!.revision).toBeGreaterThan(answer.revision);
      const off = bridge.githubDisconnect();
      await vi.advanceTimersByTimeAsync(0);
      await off;
      const refused = bridge.githubRefreshRepos();
      const check = expect(refused).rejects.toThrow(
        'Kein GitHub-Konto verbunden.',
      );
      await vi.advanceTimersByTimeAsync(0);
      await check;
    } finally {
      vi.useRealTimers();
    }
  });

  it('starts a repository in its clone folder and records it', async () => {
    const bridge = createDemoBridge(0);
    const { recent, snapshot } = await bridge.launchSession(request());
    expect(recent.folder).toBe(`${DEMO_CLONE_ROOT}\\demo-org\\kunden-portal`);
    expect(recent.repo).toEqual({ owner: 'demo-org', name: 'kunden-portal' });
    expect(snapshot.recents[0]?.id).toBe(recent.id);

    const settings = structuredClone(snapshot.settings);
    settings.github.cloneRoot = 'D:\\Code\\';
    await bridge.saveSettings(settings);
    const moved = await bridge.launchSession(request());
    expect(moved.recent.folder).toBe('D:\\Code\\demo-org\\kunden-portal');

    await expect(
      bridge.launchSession(request({ repo: { owner: '-x', name: 'y' } })),
    ).rejects.toThrow('Ungültiges Repository.');
    settings.github.cloneRoot = 'relativ';
    await expect(bridge.saveSettings(settings)).rejects.toThrow(
      'Klon-Ordner: Ordner muss ein absoluter Windows-Pfad sein.',
    );
  });

  it('restarts a repository recent in its own folder after the root moved', async () => {
    const bridge = createDemoBridge(0);
    const snapshot = await bridge.snapshot();
    const recent = snapshot.recents.find((entry) => entry.repo)!;
    const repo = recent.repo!;
    const settings = structuredClone(snapshot.settings);
    settings.github.cloneRoot = 'D:\\Code';
    await bridge.saveSettings(settings);
    const restarted = await bridge.launchSession(
      request({
        agent: recent.agent,
        env: recent.env,
        folder: recent.folder,
        repo,
        options: recent.options,
      }),
    );
    expect(restarted.recent.id).toBe(recent.id);
    expect(restarted.recent.folder).toBe(recent.folder);
    expect(restarted.recent.repo).toEqual(recent.repo);
    expect(restarted.snapshot.recents).toHaveLength(snapshot.recents.length);
    await expect(
      bridge.launchSession(request({ folder: 'relativ' })),
    ).rejects.toThrow('Ordner muss ein absoluter Windows-Pfad sein.');
  });
});
