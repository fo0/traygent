import { describe, expect, it, vi } from 'vitest';
import { createDemoBridge, demoSnapshot } from './demo';
import { DEMO_PICKED_FOLDER } from './demoData';
import type { LaunchRequest, Snapshot } from './types';

const request = (value: Partial<LaunchRequest> = {}): LaunchRequest => ({
  agent: 'codex',
  env: 'local',
  folder: 'C:\\Users\\demo\\Projekte\\neu',
  options: {
    model: 'gpt-5.5',
    effort: 'high',
    permission: 'ask',
    extraArgs: '',
  },
  session: 'new',
  ports: [],
  ...value,
});

describe('demo snapshot', () => {
  it('is labelled demo and covers every agent in local and docker', () => {
    const snapshot = demoSnapshot();
    expect(snapshot.demo).toBe(true);
    expect(snapshot.catalog.map((entry) => entry.id)).toEqual([
      'claude',
      'opencode',
      'codex',
    ]);
    expect(snapshot.agents).toHaveLength(6);
    const state = (agent: string, env: string) =>
      snapshot.agents.find((s) => s.agent === agent && s.env === env)?.state;
    expect(state('claude', 'local')).toBe('ready');
    expect(state('opencode', 'local')).toBe('missing');
    expect(state('codex', 'local')).toBe('ready');
    expect(snapshot.environments.map((env) => env.state)).toEqual([
      'ready',
      'ready',
      'unavailable',
    ]);
  });

  it('names the bypass after each CLI and opens a window per session', () => {
    const snapshot = demoSnapshot();
    const bypass = snapshot.catalog.map(
      (entry) => entry.permissions.find((p) => p.id === 'bypass')?.label,
    );
    expect(bypass).toEqual(['Bypass', 'YOLO', 'YOLO']);
    expect(snapshot.settings.sharedWindow).toBe(false);
  });

  it('lists 5–6 recents newest first, mixing agents and environments', () => {
    const { recents } = demoSnapshot(Date.parse('2026-11-03T12:00:00Z'));
    expect(recents.length).toBeGreaterThanOrEqual(5);
    expect(recents.length).toBeLessThanOrEqual(6);
    const times = recents.map((recent) => Date.parse(recent.lastStartedAt));
    expect(times).toEqual([...times].sort((a, b) => b - a));
    expect(new Set(recents.map((r) => r.agent)).size).toBe(3);
    expect(new Set(recents.map((r) => r.env))).toEqual(
      new Set(['local', 'docker']),
    );
    expect(recents.every((r) => r.folder.includes(':\\'))).toBe(true);
  });
});

describe('demo bridge', () => {
  it('hands out copies', async () => {
    const bridge = createDemoBridge(0);
    const first = await bridge.snapshot();
    first.demo = false;
    expect((await bridge.snapshot()).demo).toBe(true);
  });

  it('answers a refresh at once and pushes the finished detection', async () => {
    vi.useFakeTimers();
    try {
      const bridge = createDemoBridge(0, 500);
      const pushed: Snapshot[] = [];
      await bridge.observe((snapshot) => pushed.push(snapshot));
      const started = bridge.refreshDetection();
      await vi.advanceTimersByTimeAsync(0);
      expect((await started).detecting).toBe(true);
      expect(pushed).toEqual([]);
      await vi.advanceTimersByTimeAsync(500);
      expect(pushed).toHaveLength(1);
      expect(pushed[0]?.detecting).toBe(false);
      expect(Date.parse(pushed[0]?.checkedAt ?? '')).not.toBeNaN();
    } finally {
      vi.useRealTimers();
    }
  });

  it('records a start first and bumps an existing one', async () => {
    const bridge = createDemoBridge(0);
    const before = (await bridge.snapshot()).recents;
    const added = await bridge.launchSession(request());
    expect(added.recent).toMatchObject({ agent: 'codex', starts: 1 });
    expect(added.snapshot.recents[0]?.id).toBe(added.recent.id);
    expect(added.snapshot.recents).toHaveLength(before.length + 1);

    const old = before[3]!;
    const bumped = await bridge.launchSession(
      request({
        agent: old.agent,
        env: old.env,
        folder: old.folder,
        options: old.options,
      }),
    );
    expect(bumped.recent.id).toBe(old.id);
    expect(bumped.recent.starts).toBe(old.starts + 1);
    expect(bumped.snapshot.recents[0]?.id).toBe(old.id);
    expect(bumped.snapshot.recents).toHaveLength(before.length + 1);
  });

  it('offers the SPEC presets and records a Docker start’s normalized ports', async () => {
    const bridge = createDemoBridge(0);
    const snapshot = await bridge.snapshot();
    expect(snapshot.portPresets.map((preset) => preset.port)).toEqual([
      3000, 5173, 4200, 8000, 8080, 5000, 4321, 6006,
    ]);
    expect(snapshot.settings.docker.ports).toEqual([]);
    const withPorts = snapshot.recents.filter((r) => r.ports.length);
    expect(withPorts.map((r) => [r.env, r.ports])).toEqual([
      ['docker', ['127.0.0.1:3000:3000', '127.0.0.1:5173:5173']],
    ]);
    const { recent } = await bridge.launchSession(
      request({ env: 'docker', ports: ['3000', '8080:80/udp'] }),
    );
    expect(recent.ports).toEqual([
      '127.0.0.1:3000:3000',
      '127.0.0.1:8080:80/udp',
    ]);
    await expect(
      bridge.launchSession(request({ env: 'docker', ports: ['3000', '3000'] })),
    ).rejects.toThrow('Host-Port 3000 ist doppelt belegt.');
    await expect(
      bridge.launchSession(request({ ports: ['3000'] })),
    ).rejects.toThrow('Ports lassen sich nur für Docker-Sessions freigeben.');
    const settings = structuredClone(snapshot.settings);
    settings.docker.ports = ['5173'];
    settings.environments = ['wsl', 'local', 'wsl'];
    const saved = await bridge.saveSettings(settings);
    expect(saved.settings.docker.ports).toEqual(['127.0.0.1:5173:5173']);
    expect(saved.settings.environments).toEqual(['local', 'wsl']);
    settings.environments = [];
    await expect(bridge.saveSettings(settings)).rejects.toThrow(
      'Mindestens eine Umgebung muss aktiv sein.',
    );
  });

  it('refuses a missing CLI, a relative folder and invalid settings in German', async () => {
    const bridge = createDemoBridge(0);
    await expect(
      bridge.launchSession(request({ agent: 'opencode' })),
    ).rejects.toThrow('OpenCode ist lokal nicht installiert.');
    await expect(
      bridge.launchSession(request({ folder: 'projekte\\neu' })),
    ).rejects.toThrow('Ordner muss ein absoluter Windows-Pfad sein.');
    // Docker installs the CLI on first start: no local install needed.
    const opencode = request({ agent: 'opencode', env: 'docker' });
    opencode.options.model = 'default';
    await expect(bridge.launchSession(opencode)).resolves.toBeTruthy();
    // OpenCode names its provider, as natively.
    await expect(
      bridge.launchSession(request({ agent: 'opencode', env: 'docker' })),
    ).rejects.toThrow('Modell als provider/model angeben.');
    const settings = (await bridge.snapshot()).settings;
    settings.agents.claude.extraArgs = '--name "x"';
    await expect(bridge.saveSettings(settings)).rejects.toThrow(
      'Keine Anführungszeichen.',
    );
  });

  it('increases the revision with every change, like the native side', async () => {
    vi.useFakeTimers();
    try {
      const bridge = createDemoBridge(0, 100);
      const pushed: Snapshot[] = [];
      await bridge.observe((snapshot) => pushed.push(snapshot));
      const revision = async (step: Promise<Snapshot>) => {
        await vi.advanceTimersByTimeAsync(0);
        return (await step).revision;
      };
      const start = await revision(bridge.snapshot());
      expect(await revision(bridge.snapshot())).toBe(start);
      const detecting = await revision(bridge.refreshDetection());
      expect(detecting).toBe(start + 1);
      await vi.advanceTimersByTimeAsync(100);
      expect(pushed.map((snapshot) => snapshot.revision)).toEqual([start + 2]);
      const current = bridge.snapshot();
      await vi.advanceTimersByTimeAsync(0);
      const { settings } = await current;
      expect(await revision(bridge.saveSettings(settings))).toBe(start + 3);
      const launch = bridge.launchSession(request());
      await vi.advanceTimersByTimeAsync(0);
      const { recent, snapshot } = await launch;
      expect(snapshot.revision).toBe(start + 4);
      expect(await revision(bridge.removeRecent(recent.id))).toBe(start + 5);
      expect(await revision(bridge.restoreRecent(recent))).toBe(start + 6);
      expect(
        await revision(bridge.updateRecent(recent.id, recent.options, [])),
      ).toBe(start + 7);
    } finally {
      vi.useRealTimers();
    }
  });

  it('restores a removed recent only if it was not started again', async () => {
    const bridge = createDemoBridge(0);
    const before = (await bridge.snapshot()).recents;
    const removed = before[2]!;
    await bridge.removeRecent(removed.id);
    const again = await bridge.launchSession(
      request({
        agent: removed.agent,
        env: removed.env,
        folder: removed.folder,
        options: removed.options,
      }),
    );
    const restored = await bridge.restoreRecent(removed);
    expect(restored.recents.filter((r) => r.id === removed.id)).toEqual([
      again.recent,
    ]);
    await expect(
      bridge.restoreRecent({ ...removed, lastStartedAt: 'gestern' }),
    ).rejects.toThrow('Eintrag kann nicht wiederhergestellt werden.');
  });

  it('removes and restores a recent at its place', async () => {
    const bridge = createDemoBridge(0);
    const before = (await bridge.snapshot()).recents;
    const removed = before[2]!;
    const after = await bridge.removeRecent(removed.id);
    expect(after.recents.map((r) => r.id)).not.toContain(removed.id);
    const restored = await bridge.restoreRecent(removed);
    expect(restored.recents.map((r) => r.id)).toEqual(before.map((r) => r.id));
  });

  it('edits a recent in place and refuses what a start refuses', async () => {
    const bridge = createDemoBridge(0);
    const before = (await bridge.snapshot()).recents;
    const docker = before.find((r) => r.env === 'docker')!;
    const local = before.find((r) => r.env === 'local')!;
    const options = {
      ...docker.options,
      permission: 'bypass' as const,
      extraArgs: ' --verbose ',
    };
    const after = await bridge.updateRecent(docker.id, options, [' 3000 ']);
    expect(after.recents.map((r) => r.id)).toEqual(before.map((r) => r.id));
    const edited = after.recents.find((r) => r.id === docker.id)!;
    expect(edited.options).toEqual({ ...options, extraArgs: '--verbose' });
    expect(edited.ports).toEqual(['127.0.0.1:3000:3000']);
    expect(edited.lastStartedAt).toBe(docker.lastStartedAt);
    expect(edited.starts).toBe(docker.starts);
    await expect(
      bridge.updateRecent(local.id, local.options, ['3000']),
    ).rejects.toThrow('Ports lassen sich nur für Docker-Sessions freigeben.');
    await expect(
      bridge.updateRecent(local.id, { ...local.options, extraArgs: '"x"' }, []),
    ).rejects.toThrow('Extra-Argumente: Keine Anführungszeichen.');
    await expect(bridge.updateRecent('weg', local.options, [])).rejects.toThrow(
      'Diese Session steht nicht mehr in „Zuletzt“.',
    );
  });

  it('saves settings, picks a fake folder and opens nothing', async () => {
    const bridge = createDemoBridge(0);
    const settings = (await bridge.snapshot()).settings;
    settings.defaultEnv = 'docker';
    settings.sharedWindow = true;
    const saved = (await bridge.saveSettings(settings)).settings;
    expect(saved.defaultEnv).toBe('docker');
    expect(saved.sharedWindow).toBe(true);
    expect(await bridge.pickFolder(null)).toBe(DEMO_PICKED_FOLDER);
    await expect(bridge.installAgent('opencode')).resolves.toBeUndefined();
    await expect(bridge.openFolder('C:\\x')).resolves.toBeUndefined();
  });
});
