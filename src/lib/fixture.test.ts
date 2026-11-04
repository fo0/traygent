import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { startState } from './agents';
import { Composer } from './composerState.svelte';
import { envsProblem } from './environments';
import { portListProblem } from './ports';
import type { Snapshot } from './types';

/*
 * The golden fixture a Rust test writes (fixtures/snapshot.default.json) must
 * satisfy `Snapshot` in src/lib/types.ts. JSON carries no types, so each
 * object is checked against the contract's required and optional keys and
 * every union against its values.
 */
const fixture: Record<string, unknown> = JSON.parse(
  readFileSync(join(process.cwd(), 'fixtures/snapshot.default.json'), 'utf8'),
);

const AGENTS = ['claude', 'opencode', 'codex'];
const ENVS = ['local', 'docker', 'wsl'];
const PERMISSIONS = ['ask', 'auto', 'bypass'];
const SESSIONS = ['new', 'continue', 'resume'];
const STATES = ['ready', 'missing', 'stopped', 'unavailable', 'unknown'];
const TERMINALS = ['windowsTerminal', 'console'];

type Shape = { required: string[]; optional?: string[] };
function expectShape(value: unknown, shape: Shape, where: string) {
  expect(value, where).toBeTypeOf('object');
  const keys = Object.keys(value as object);
  for (const key of shape.required)
    expect(keys, `${where}.${key}`).toContain(key);
  const known = [...shape.required, ...(shape.optional ?? [])];
  expect(
    keys.filter((key) => !known.includes(key)),
    `${where}: keys outside the contract`,
  ).toEqual([]);
}
const OPTIONS: Shape = {
  required: ['model', 'effort', 'permission', 'extraArgs'],
};
const CHOICE: Shape = { required: ['id', 'label'], optional: ['hint'] };

describe('golden snapshot fixture', () => {
  it('has exactly the Snapshot keys and is never demo', () => {
    expectShape(
      fixture,
      {
        required: [
          'demo',
          'settings',
          'catalog',
          'portPresets',
          'agents',
          'environments',
          'recents',
          'github',
          'detecting',
          'revision',
        ],
        optional: ['notice', 'checkedAt'],
      },
      'snapshot',
    );
    expect(fixture.demo).toBe(false);
    expect(fixture.detecting).toBeTypeOf('boolean');
    expect(Number.isSafeInteger(fixture.revision), 'revision').toBe(true);
    expect(fixture.revision as number).toBeGreaterThanOrEqual(0);
    if ('notice' in fixture) expect(fixture.notice).toBeTypeOf('string');
    expect(Array.isArray(fixture.recents)).toBe(true);
  });

  it('carries valid settings', () => {
    const settings = (fixture as unknown as Snapshot).settings;
    expectShape(
      settings,
      {
        required: [
          'defaultAgent',
          'defaultEnv',
          'environments',
          'terminal',
          'sharedWindow',
          'agents',
          'docker',
          'github',
        ],
      },
      'settings',
    );
    expectShape(settings.github, { required: ['cloneRoot'] }, 'github');
    expect(settings.github.cloneRoot).toBeTypeOf('string');
    expect(AGENTS).toContain(settings.defaultAgent);
    expect(ENVS).toContain(settings.defaultEnv);
    // Every environment, in order: a fresh install offers what it always did.
    expect(settings.environments).toEqual(ENVS);
    expect(envsProblem(settings), 'environments').toBe('');
    expect(TERMINALS).toContain(settings.terminal);
    // A fresh install opens a window per session, as before.
    expect(settings.sharedWindow).toBe(false);
    expect(Object.keys(settings.agents).sort()).toEqual([...AGENTS].sort());
    for (const agent of AGENTS) {
      const options = settings.agents[agent as keyof typeof settings.agents];
      expectShape(options, OPTIONS, `settings.agents.${agent}`);
      expect(PERMISSIONS).toContain(options.permission);
    }
    expectShape(
      settings.docker,
      { required: ['images', 'ghToken', 'ports'] },
      'docker',
    );
    expect(portListProblem(settings.docker.ports), 'docker.ports').toBe('');
    expect(Object.keys(settings.docker.images).sort()).toEqual(
      [...AGENTS].sort(),
    );
  });

  it('lists a renderable catalog entry per agent', () => {
    const catalog = (fixture as unknown as Snapshot).catalog;
    expect(catalog.map((entry) => entry.id)).toEqual(AGENTS);
    for (const entry of catalog) {
      expectShape(
        entry,
        {
          required: [
            'id',
            'label',
            'models',
            'freeModel',
            'efforts',
            'permissions',
            'sessions',
            'environments',
            'install',
          ],
        },
        entry.id,
      );
      expect(entry.models[0]?.id, `${entry.id}: first model`).toBe('default');
      for (const choice of [...entry.models, ...entry.efforts])
        expectShape(choice, CHOICE, `${entry.id} choice`);
      expect(entry.permissions.map((p) => p.id)).toEqual(PERMISSIONS);
      for (const permission of entry.permissions)
        expectShape(
          permission,
          { required: ['id', 'label', 'hint', 'danger'] },
          `${entry.id} permission`,
        );
      expect(entry.sessions).toContain('new');
      for (const mode of entry.sessions) expect(SESSIONS).toContain(mode);
      for (const env of entry.environments) expect(ENVS).toContain(env);
      expectShape(
        entry.install,
        { required: ['command', 'docsUrl'], optional: ['update', 'requires'] },
        `${entry.id} install`,
      );
    }
  });

  it('offers the port presets of docs/SPEC.md → Ports', () => {
    const presets = (fixture as unknown as Snapshot).portPresets;
    for (const preset of presets)
      expectShape(preset, { required: ['port', 'label'] }, 'port preset');
    expect(presets.map((preset) => preset.port)).toEqual([
      3000, 5173, 4200, 8000, 8080, 5000, 4321, 6006,
    ]);
    expect(presets.every((preset) => preset.label.length > 0)).toBe(true);
  });

  it('starts without a GitHub account and an empty listing', () => {
    const github = (fixture as unknown as Snapshot).github;
    expectShape(
      github,
      {
        required: ['repos', 'loading', 'defaultCloneRoot'],
        optional: ['account', 'error', 'listedAt'],
      },
      'github',
    );
    expect(github.account).toBeUndefined();
    expect(github.repos).toEqual([]);
    expect(github.loading).toBeTypeOf('boolean');
    expect(github.defaultCloneRoot).toBeTypeOf('string');
  });

  it('probes every agent locally and in Docker, and every environment', () => {
    const snapshot = fixture as unknown as Snapshot;
    for (const status of snapshot.agents) {
      expectShape(
        status,
        {
          required: ['agent', 'env', 'state'],
          optional: ['version', 'path', 'detail'],
        },
        'agent status',
      );
      expect(STATES).toContain(status.state);
    }
    const pairs = snapshot.agents.map((s) => `${s.agent}:${s.env}`).sort();
    expect(pairs).toEqual(
      AGENTS.flatMap((agent) => [`${agent}:docker`, `${agent}:local`]).sort(),
    );
    expect(snapshot.environments.map((env) => env.env)).toEqual(ENVS);
    for (const env of snapshot.environments) {
      expectShape(
        env,
        { required: ['env', 'state'], optional: ['version', 'detail'] },
        'environment',
      );
      expect(STATES).toContain(env.state);
    }
  });

  it('drives the composer like the demo does', () => {
    const snapshot = fixture as unknown as Snapshot;
    const composer = new Composer();
    composer.sync(snapshot);
    expect(composer.agent).toBe(snapshot.settings.defaultAgent);
    expect(composer.options).toEqual(
      snapshot.settings.agents[snapshot.settings.defaultAgent],
    );
    expect(startState(snapshot, { ...composer.request(), folder: '' })).toEqual(
      { kind: 'blocked', reason: 'Ordner wählen' },
    );
  });
});
