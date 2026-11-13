import { describe, expect, it, vi } from 'vitest';
import { createDemoBridge, demoSnapshot } from './demo';
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
});
