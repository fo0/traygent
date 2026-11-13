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
  it('lists Claude Code’s „Standard“ and „Team“, signed in with invented accounts', () => {
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
      snapshot.profiles.map((p) => [p.id, p.label, p.signedIn, p.account]),
    ).toEqual([
      ['default', 'Standard', true, 'demo@example.com'],
      ['team', 'Team', true, 'team@example.com'],
    ]);
    expect(snapshot.recents.filter((r) => r.profile)).toHaveLength(1);
  });

  it('records a start’s profile and refuses one that is gone', async () => {
    const bridge = createDemoBridge(0);
    const outcome = await bridge.launchSession({ ...start, profile: 'team' });
    expect(outcome.recent.profile).toBe('team');
    expect(outcome).not.toHaveProperty('warning');
    await expect(
      bridge.launchSession({ ...start, profile: 'weg' }),
    ).rejects.toThrow('Profil „weg“ gibt es nicht mehr.');
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
      ['team', true],
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

  it('syncs every extra profile and signs one in like a detection', async () => {
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
    await expect(
      bridge.profileLogin('claude', 'kunde'),
    ).resolves.toBeUndefined();
    expect(pushed).toEqual([]);
    await vi.waitFor(() =>
      expect(pushed).toEqual(['default:true team:true kunde:true']),
    );
    const after = await bridge.snapshot();
    expect(after.profiles[2]?.account).toBe('kunde@example.com');
    await expect(bridge.profileLogin('codex', 'default')).rejects.toThrow(
      'Dieses Profil gibt es nicht.',
    );
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
      ['team', true],
      ['kunde', false],
    ]);
    await expect(
      bridge.profileLogin('claude', 'kunde'),
    ).resolves.toBeUndefined();
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
      signedIn: true,
    });
  });
});
