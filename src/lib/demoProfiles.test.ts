import { describe, expect, it, vi } from 'vitest';
import { createDemoBridge, demoSnapshot } from './demo';
import { SettingsDraft } from './settingsDraft.svelte';
import type { LaunchRequest } from './types';

const start: LaunchRequest = {
  agent: 'claude',
  env: 'local',
  folder: 'C:\\Users\\demo\\Projekte\\neu',
  options: {
    model: 'default',
    effort: 'default',
    permission: 'auto',
    extraArgs: '',
  },
  session: 'new',
  ports: [],
};

describe('demo profiles', () => {
  it('lists Claude Code’s „Standard“, signed in with an invented account, and „Team“, signed out', () => {
    const snapshot = demoSnapshot();
    expect(snapshot.catalog.map((entry) => entry.profiles)).toEqual([
      true,
      false,
      false,
    ]);
    expect(snapshot.settings.profiles).toEqual({
      claude: [{ id: 'team', label: 'Team' }],
      opencode: [],
      codex: [],
    });
    expect(snapshot.settings.agentProfile.claude).toBe('default');
    expect(snapshot.settings.profileShare).toEqual({
      claude: { dirs: ['.caveman-sessions'], files: ['orca-state.json'] },
      opencode: { dirs: [], files: [] },
      codex: { dirs: [], files: [] },
    });
    expect(
      snapshot.profiles.map((p) => [
        p.id,
        p.label,
        p.signedIn,
        p.account,
        p.organization,
      ]),
    ).toEqual([
      ['default', 'Standard', true, 'demo@example.com', 'Beispiel GmbH'],
      ['team', 'Team', false, undefined, undefined],
    ]);
    expect(snapshot.settings.debug).toBe(false);
    expect(snapshot.recents.filter((r) => r.profile)).toHaveLength(1);
  });

  it('records a start’s profile and refuses one that is gone, as natively', async () => {
    const bridge = createDemoBridge(0);
    const outcome = await bridge.launchSession({ ...start, profile: ' team ' });
    expect(outcome.recent.profile).toBe('team');
    expect(outcome).not.toHaveProperty('warning');
    await expect(
      bridge.launchSession({ ...start, profile: 'weg' }),
    ).rejects.toThrow('Profil nicht gefunden.');
    // A container has its own home: the profile is dropped, not refused.
    const docker = await bridge.launchSession({
      ...start,
      env: 'docker',
      profile: 'team',
    });
    expect(docker.recent).not.toHaveProperty('profile');
  });

  it('saves a new profile signed out and keeps the others’ sign-in', async () => {
    const bridge = createDemoBridge(0);
    const { settings } = await bridge.snapshot();
    settings.profiles.claude.push({ id: 'kunde', label: 'Kunde' });
    settings.agentProfile.claude = 'kunde';
    const saved = await bridge.saveSettings(settings);
    expect(saved.settings.agentProfile.claude).toBe('kunde');
    expect(saved.profiles.map((p) => [p.id, p.signedIn])).toEqual([
      ['default', true],
      ['team', false],
      ['kunde', false],
    ]);
    expect(saved.profiles[2]?.dir).toBe('C:\\Users\\demo\\.claude-kunde');
    settings.agentProfile.claude = 'weg';
    await expect(bridge.saveSettings(settings)).rejects.toThrow(
      'Standard-Profil gibt es nicht.',
    );
  });

  it('saves shared names and refuses one the sync handles, named natively', async () => {
    const bridge = createDemoBridge(0);
    const { settings } = await bridge.snapshot();
    settings.profileShare.claude.files.push('notes.md');
    const saved = await bridge.saveSettings(settings);
    expect(saved.settings.profileShare.claude.files).toEqual([
      'orca-state.json',
      'notes.md',
    ]);
    settings.profileShare.claude.dirs.push('Skills');
    await expect(bridge.saveSettings(settings)).rejects.toThrow(
      'Name „Skills“ für Claude Code wird bereits abgeglichen.',
    );
  });

  it('syncs every extra profile and signs one in hidden: it resolves signed in', async () => {
    const bridge = createDemoBridge(0, 10);
    const pushed: string[] = [];
    await bridge.observe((value) =>
      pushed.push(value.profiles.map((p) => `${p.id}:${p.signedIn}`).join(' ')),
    );
    const settings = (await bridge.snapshot()).settings;
    settings.profiles.claude.push({ id: 'kunde', label: 'Kunde' });
    const before = (await bridge.saveSettings(settings)).profiles;
    const synced = (await bridge.syncProfiles()).profiles;
    expect(synced[0]).toEqual(before[0]);
    expect(synced.slice(1).every((p) => p.syncedAt && !p.syncError)).toBe(true);
    const signed = await bridge.profileLogin('claude', 'kunde');
    // The same account as „Standard“, an organization of its own; no event.
    expect(signed.profiles[2]).toMatchObject({
      signedIn: true,
      account: 'demo@example.com',
      organization: 'Beispiel Kunde',
    });
    expect(pushed).toEqual([]);
    await expect(bridge.profileLogin('codex', 'default')).rejects.toThrow(
      'Profil nicht gefunden.',
    );
  });

  it('refuses a second sign-in of a running profile and cancels one', async () => {
    const bridge = createDemoBridge(0, 50);
    const running = bridge.profileLogin('claude', 'team');
    await new Promise((resolve) => setTimeout(resolve, 5));
    await expect(bridge.profileLogin('claude', 'team')).rejects.toThrow(
      'Die Anmeldung läuft bereits.',
    );
    expect(await bridge.cancelProfileLogin('claude', 'team')).toBe(true);
    await expect(running).rejects.toThrow('Anmeldung abgebrochen.');
    expect((await bridge.snapshot()).profiles[1]?.signedIn).toBe(false);
    // Nothing runs: a cancel finds nothing, the next sign-in starts afresh.
    expect(await bridge.cancelProfileLogin('claude', 'team')).toBe(false);
    expect(
      (await bridge.profileLogin('claude', 'team')).profiles[1],
    ).toMatchObject({ signedIn: true, organization: 'Beispiel Team' });
  });

  it('with „Debug“ the terminal „opens“ at once and the sign-in follows as an event', async () => {
    const bridge = createDemoBridge(0, 10);
    const pushed: string[] = [];
    await bridge.observe((value) =>
      pushed.push(value.profiles.map((p) => `${p.id}:${p.signedIn}`).join(' ')),
    );
    const { settings } = await bridge.snapshot();
    settings.debug = true;
    await bridge.saveSettings(settings);
    const opened = await bridge.profileLogin('claude', 'team');
    expect(opened.profiles[1]?.signedIn).toBe(false);
    await vi.waitFor(() => expect(pushed).toEqual(['default:true team:true']));
  });

  it('stores an edited profile as natively', async () => {
    const bridge = createDemoBridge(0);
    const before = (await bridge.snapshot()).recents;
    const local = before.find(
      (r) => r.agent === 'claude' && r.env === 'local',
    )!;
    const docker = before.find((r) => r.env === 'docker')!;
    const codex = before.find((r) => r.agent === 'codex')!;
    const stored = async (id: string, profile?: string) => {
      const { options } = before.find((r) => r.id === id)!;
      const { recents } = await bridge.updateRecent(id, options, [], profile);
      return recents.find((r) => r.id === id)!.profile;
    };
    expect(local.profile).toBeUndefined();
    expect(await stored(local.id, ' team ')).toBe('team');
    expect(await stored(local.id, 'default')).toBeUndefined();
    await stored(local.id, 'team');
    expect(await stored(local.id)).toBeUndefined();
    // A container has its own home: the profile is dropped, not refused.
    expect(await stored(docker.id, 'team')).toBeUndefined();
    for (const [recent, profile] of [
      [local, 'gone'],
      [codex, 'team'],
    ] as const)
      await expect(
        bridge.updateRecent(recent.id, recent.options, [], profile),
      ).rejects.toThrow('Profil nicht gefunden.');
  });

  it('adds a profile at once through the draft: it signs in before „Speichern“', async () => {
    const bridge = createDemoBridge(0, 10);
    const draft = new SettingsDraft((await bridge.snapshot()).settings);
    draft.value.sharedWindow = true;
    const save = async (value: typeof draft.saved) =>
      (await bridge.saveSettings(value)).settings;
    expect(await draft.addProfile('claude', 'Kunde', save)).toBe('');
    const shown = await bridge.snapshot();
    expect(shown.settings.profiles.claude.map((p) => p.id)).toEqual([
      'team',
      'kunde',
    ]);
    // The staged edit was not sent and stays staged.
    expect(shown.settings.sharedWindow).toBe(false);
    expect(draft.current().sharedWindow).toBe(true);
    expect(shown.profiles.map((p) => [p.id, p.signedIn])).toEqual([
      ['default', true],
      ['team', false],
      ['kunde', false],
    ]);
    const signed = await bridge.profileLogin('claude', 'kunde');
    expect(signed.profiles[2]?.signedIn).toBe(true);
  });

  it('renames a profile on „Speichern“: id, folder and sign-in stay', async () => {
    const bridge = createDemoBridge(0);
    const { settings } = await bridge.snapshot();
    settings.profiles.claude[0]!.label = 'Kunde B';
    const saved = await bridge.saveSettings(settings);
    expect(saved.profiles[1]).toMatchObject({
      id: 'team',
      label: 'Kunde B',
      dir: 'C:\\Users\\demo\\.claude-team',
      signedIn: false,
    });
  });
});
