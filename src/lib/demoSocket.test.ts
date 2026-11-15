import { describe, expect, it } from 'vitest';
import { createDemoBridge } from './demo';
import type { LaunchRequest } from './types';

/* The host Docker socket (#46) in the labelled demo, as natively. */
const request = (value: Partial<LaunchRequest> = {}): LaunchRequest => ({
  agent: 'claude',
  env: 'docker',
  folder: 'C:\\Users\\demo\\Projekte\\neu',
  options: {
    model: 'default',
    effort: 'default',
    permission: 'auto',
    extraArgs: '',
  },
  session: 'new',
  ports: [],
  ...value,
});

describe('demo Docker socket', () => {
  it('starts off in the settings and shows one Docker row with it', async () => {
    const snapshot = await createDemoBridge(0).snapshot();
    expect(snapshot.settings.docker.socket).toBe(false);
    const rows = snapshot.recents.filter((recent) => recent.dockerSocket);
    expect(rows.map((recent) => recent.env)).toEqual(['docker']);
  });

  it('saves the setting staged like the others', async () => {
    const bridge = createDemoBridge(0);
    const settings = (await bridge.snapshot()).settings;
    settings.docker.socket = true;
    const saved = await bridge.saveSettings(settings);
    expect(saved.settings.docker.socket).toBe(true);
  });

  it('records the socket of a Docker start only', async () => {
    const bridge = createDemoBridge(0);
    const docker = await bridge.launchSession(request({ dockerSocket: true }));
    expect(docker.recent.dockerSocket).toBe(true);
    const local = await bridge.launchSession(
      request({ env: 'local', dockerSocket: true }),
    );
    expect(local.recent).not.toHaveProperty('dockerSocket');
    const again = await bridge.launchSession(request());
    expect(again.recent).not.toHaveProperty('dockerSocket');
  });

  it('stores an edited socket on a Docker row and ignores it elsewhere', async () => {
    const bridge = createDemoBridge(0);
    const before = (await bridge.snapshot()).recents;
    const docker = before.find((r) => r.env === 'docker' && !r.dockerSocket)!;
    const local = before.find((r) => r.env === 'local')!;
    const edit = async (id: string, socket: boolean) => {
      const { options, ports, profile } = before.find((r) => r.id === id)!;
      const { recents } = await bridge.updateRecent(
        id,
        options,
        ports,
        profile,
        false,
        socket,
      );
      return recents.find((r) => r.id === id)!;
    };
    expect((await edit(docker.id, true)).dockerSocket).toBe(true);
    expect(await edit(docker.id, false)).not.toHaveProperty('dockerSocket');
    expect(await edit(local.id, true)).not.toHaveProperty('dockerSocket');
  });
});
