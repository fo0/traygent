import { describe, expect, it } from 'vitest';
import {
  catalogEntry,
  choiceTitle,
  dockerLine,
  effortChoices,
  effortNeedsModel,
  folderName,
  nextPermission,
  parentPath,
  sentOptions,
  shimStart,
  shortVersion,
  startState,
} from './agents';
import { DEMO_CATALOG, demoSnapshot } from './demoData';
import type { EnvStatus, LaunchOptions, ProbeState, Snapshot } from './types';

const permissions = DEMO_CATALOG[0]!.permissions;

describe('permission cycle', () => {
  it('cycles ask → auto → bypass → ask in catalog order', () => {
    expect(nextPermission('ask', permissions)).toBe('auto');
    expect(nextPermission('auto', permissions)).toBe('bypass');
    expect(nextPermission('bypass', permissions)).toBe('ask');
  });
  it('falls back to the unified order without a catalog', () => {
    expect(nextPermission('auto', [])).toBe('bypass');
  });
  it('follows a shorter catalog list', () => {
    const two = permissions.filter((choice) => choice.id !== 'auto');
    expect(nextPermission('ask', two)).toBe('bypass');
    expect(nextPermission('bypass', two)).toBe('ask');
  });
});

describe('Windows path parts', () => {
  it('splits folder name and parent', () => {
    expect(folderName('C:\\Users\\demo\\app')).toBe('app');
    expect(parentPath('C:\\Users\\demo\\app')).toBe('C:\\Users\\demo');
    expect(folderName('C:\\Users\\demo\\app\\')).toBe('app');
    expect(parentPath('D:\\work')).toBe('D:\\');
  });
  it('keeps drive roots and UNC shares readable', () => {
    expect(folderName('C:\\')).toBe('C:\\');
    expect(parentPath('C:\\')).toBe('');
    expect(folderName('\\\\server\\share\\repo')).toBe('repo');
    expect(parentPath('\\\\server\\share\\repo')).toBe('\\\\server\\share');
  });
});

describe('Docker line and versions', () => {
  it('shortens the engine version', () => {
    expect(shortVersion('27.5.1')).toBe('27.5');
    expect(shortVersion('28.0.0-rc.1')).toBe('28.0');
    expect(shortVersion(undefined)).toBe('');
  });
  it('names every probe state in German', () => {
    expect(
      dockerLine({ env: 'docker', state: 'ready', version: '27.5.1' }).text,
    ).toBe('Docker 27.5 · bereit');
    expect(dockerLine({ env: 'docker', state: 'stopped' }).text).toBe(
      'Docker nicht erreichbar',
    );
    expect(dockerLine({ env: 'docker', state: 'missing' }).text).toBe(
      'Docker fehlt',
    );
    expect(dockerLine(undefined).state).toBe('unknown');
  });
});

describe('startState', () => {
  const snapshot = demoSnapshot();
  const folder = 'C:\\Users\\demo\\Projekte\\traygent';

  it('is ready for an installed agent with a folder', () => {
    expect(
      startState(snapshot, { agent: 'claude', env: 'local', folder }),
    ).toEqual({ kind: 'ready' });
  });
  it('turns a missing local CLI into an install, even without a folder', () => {
    expect(
      startState(snapshot, { agent: 'opencode', env: 'local', folder: '' }),
    ).toEqual({ kind: 'install' });
  });
  it('lets Docker install the CLI itself', () => {
    expect(
      startState(snapshot, { agent: 'opencode', env: 'docker', folder }),
    ).toEqual({ kind: 'ready' });
  });
  it('blocks without a folder, in WSL and with Docker down', () => {
    expect(
      startState(snapshot, { agent: 'claude', env: 'local', folder: ' ' }),
    ).toEqual({ kind: 'blocked', reason: 'Ordner wählen' });
    expect(
      startState(snapshot, { agent: 'claude', env: 'wsl', folder }),
    ).toEqual({ kind: 'blocked', reason: 'Bald verfügbar' });
    const stopped: Snapshot = {
      ...snapshot,
      environments: snapshot.environments.map((env) =>
        env.env === 'docker' ? { env: 'docker', state: 'stopped' } : env,
      ),
    };
    expect(
      startState(stopped, { agent: 'claude', env: 'docker', folder }),
    ).toEqual({ kind: 'blocked', reason: 'Docker nicht erreichbar' });
  });

  const withDocker = (docker: EnvStatus): Snapshot => ({
    ...snapshot,
    environments: snapshot.environments.map((env) =>
      env.env === 'docker' ? docker : env,
    ),
  });
  it('names a Docker probe’s own reason, never „Bald verfügbar“', () => {
    const windows = withDocker({
      env: 'docker',
      state: 'unavailable',
      version: '27.5.1',
      detail: 'Docker nutzt Windows-Container – auf Linux-Container umschalten',
    });
    expect(
      startState(windows, { agent: 'codex', env: 'docker', folder }),
    ).toEqual({
      kind: 'blocked',
      reason: 'Docker nutzt Windows-Container – auf Linux-Container umschalten',
    });
    const states: [ProbeState, string][] = [
      ['missing', 'Docker fehlt'],
      ['stopped', 'Docker nicht erreichbar'],
      ['unavailable', 'Docker nicht verfügbar'],
    ];
    for (const [state, line] of states) {
      expect(
        startState(withDocker({ env: 'docker', state }), {
          agent: 'claude',
          env: 'docker',
          folder,
        }),
        state,
      ).toEqual({ kind: 'blocked', reason: line });
      expect(
        startState(
          withDocker({ env: 'docker', state, detail: 'Docker läuft nicht' }),
          { agent: 'claude', env: 'docker', folder },
        ),
        state,
      ).toEqual({ kind: 'blocked', reason: 'Docker läuft nicht' });
    }
    // Docker being down never blocks a local start.
    expect(
      startState(withDocker({ env: 'docker', state: 'unavailable' }), {
        agent: 'claude',
        env: 'local',
        folder,
      }),
    ).toEqual({ kind: 'ready' });
  });
  it('keeps „Bald verfügbar“ for WSL only, whatever its probe says', () => {
    const ready: Snapshot = {
      ...snapshot,
      environments: snapshot.environments.map((env) =>
        env.env === 'wsl' ? { env: 'wsl', state: 'ready' } : env,
      ),
    };
    expect(startState(ready, { agent: 'claude', env: 'wsl', folder })).toEqual({
      kind: 'blocked',
      reason: 'Bald verfügbar',
    });
  });
  it('offers the install for a local CLI that cannot start', () => {
    const broken: Snapshot = {
      ...snapshot,
      agents: snapshot.agents.map((status) =>
        status.agent === 'claude' && status.env === 'local'
          ? { ...status, state: 'unavailable' }
          : status,
      ),
    };
    expect(
      startState(broken, { agent: 'claude', env: 'local', folder }),
    ).toEqual({ kind: 'install' });
  });
});

describe('effort and hints from the catalog', () => {
  const snapshot = demoSnapshot();
  const claude = catalogEntry(snapshot, 'claude');
  const opencode = catalogEntry(snapshot, 'opencode');
  const options = (model: string, effort: string): LaunchOptions => ({
    model,
    effort,
    permission: 'ask',
    extraArgs: '',
  });

  it('offers an effort only where the model can carry it', () => {
    expect(effortChoices(claude, 'default')).toEqual(claude?.efforts);
    expect(effortChoices(opencode, 'default')).toEqual([]);
    expect(effortChoices(opencode, 'opencode/big-pickle')).toEqual(
      opencode?.efforts,
    );
    expect(effortChoices(undefined, 'opus')).toEqual([]);
    expect(effortNeedsModel(opencode, 'default')).toBe(true);
    expect(effortNeedsModel(opencode, 'a/b')).toBe(false);
    expect(effortNeedsModel(claude, 'default')).toBe(false);
  });
  it('sends and records `default` for an effort that was not applicable', () => {
    expect(sentOptions(opencode, options('default', 'high'))).toEqual(
      options('default', 'default'),
    );
    expect(sentOptions(opencode, options('a/b', 'high'))).toEqual(
      options('a/b', 'high'),
    );
    expect(sentOptions(claude, options('default', 'max'))).toEqual(
      options('default', 'max'),
    );
  });
  it('puts the catalog hint into the tooltip', () => {
    expect(choiceTitle('Modell', claude!.models, 'default')).toBe(
      'Modell: Standard – Ohne --model: Standard des Kontos',
    );
    expect(choiceTitle('Modell', claude!.models, 'opus')).toBe('Modell: Opus');
    expect(choiceTitle('Modell', claude!.models, 'my-model')).toBe(
      'Modell: my-model',
    );
    expect(choiceTitle('Denken', claude!.efforts, 'default')).toBe(
      'Denken: Standard – Ohne --effort: Standard des Modells',
    );
  });
  it('knows a local start through an npm `.cmd` shim', () => {
    const shim: Snapshot = {
      ...snapshot,
      agents: snapshot.agents.map((status) =>
        status.agent === 'codex' && status.env === 'local'
          ? { ...status, path: 'C:\\npm\\codex.CMD' }
          : status,
      ),
    };
    expect(shimStart(shim, 'codex', 'local')).toBe(true);
    expect(shimStart(shim, 'codex', 'docker')).toBe(false);
    expect(shimStart(snapshot, 'claude', 'local')).toBe(false);
  });
});
