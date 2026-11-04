import { describe, expect, it } from 'vitest';
import { Composer, recentTarget } from './composerState.svelte';
import { demoSnapshot } from './demoData';
import type { Snapshot } from './types';

const loaded = () => {
  const composer = new Composer();
  const snapshot = demoSnapshot();
  composer.sync(snapshot);
  return { composer, snapshot };
};

describe('Composer', () => {
  it('starts from the saved defaults with an empty folder field', () => {
    const { composer, snapshot } = loaded();
    expect(composer.ready).toBe(true);
    expect(composer.agent).toBe(snapshot.settings.defaultAgent);
    expect(composer.env).toBe(snapshot.settings.defaultEnv);
    expect(composer.folder).toBe('');
    expect(composer.options).toEqual(snapshot.settings.agents.claude);
    expect(composer.session).toBe('new');
  });

  it('loads the agent’s defaults on an agent change and limits the sessions', () => {
    const { composer, snapshot } = loaded();
    composer.session = 'resume';
    composer.selectAgent('opencode');
    expect(composer.options).toEqual(snapshot.settings.agents.opencode);
    expect(composer.session).toBe('new');
    expect(composer.sessions).toEqual(['new', 'continue']);
    composer.session = 'resume';
    // A mode the agent lacks never reaches the native side.
    expect(composer.request().session).toBe('new');
  });

  it('changes only the next start, never the defaults', () => {
    const { composer, snapshot } = loaded();
    composer.options.model = 'opus';
    composer.cyclePermission();
    expect(composer.options.permission).toBe('bypass');
    composer.cyclePermission();
    expect(composer.options.permission).toBe('ask');
    expect(snapshot.settings.agents.claude.model).toBe('default');
    const request = composer.request();
    expect(request).toMatchObject({
      agent: 'claude',
      env: 'local',
      options: { model: 'opus', permission: 'ask' },
      session: 'new',
    });
    // The request is a plain copy, detached from the form.
    composer.options.model = 'haiku';
    expect(request.options.model).toBe('opus');
    composer.launched();
    expect(composer.options).toEqual(snapshot.settings.agents.claude);
  });

  it('keeps the user’s choices across snapshots, applying only changed defaults', () => {
    const { composer, snapshot } = loaded();
    composer.selectEnv('docker');
    composer.folder = 'D:\\Arbeit\\neu';
    composer.options.effort = 'max';
    composer.sync({ ...snapshot, detecting: true });
    expect(composer.env).toBe('docker');
    expect(composer.folder).toBe('D:\\Arbeit\\neu');
    expect(composer.options.effort).toBe('max');

    // A saved dialog: the changed defaults apply, the folder stays.
    const settings = structuredClone(snapshot.settings);
    settings.agents.claude.permission = 'bypass';
    settings.defaultEnv = 'wsl';
    const saved: Snapshot = { ...snapshot, settings };
    composer.sync(saved);
    expect(composer.env).toBe('wsl');
    expect(composer.folder).toBe('D:\\Arbeit\\neu');
    expect(composer.options.permission).toBe('bypass');
    expect(composer.options.effort).toBe('default');
  });

  it('starts in the first shown environment when the default is hidden', () => {
    const composer = new Composer();
    const snapshot = demoSnapshot();
    snapshot.settings.environments = ['docker', 'wsl'];
    composer.sync(snapshot);
    expect(composer.environments).toEqual(['docker', 'wsl']);
    expect(composer.env).toBe('docker');
  });

  it('falls back from an environment a saved dialog hid, also for a recent', () => {
    const { composer, snapshot } = loaded();
    composer.selectEnv('docker');
    const settings = structuredClone(snapshot.settings);
    settings.environments = ['local'];
    composer.sync({ ...snapshot, settings });
    expect(composer.environments).toEqual(['local']);
    expect(composer.env).toBe('local');
    // The recent keeps its env; adopting it into the form uses a shown one.
    const recent = snapshot.recents.find((item) => item.env === 'docker')!;
    composer.adopt(recent);
    expect(recent.env).toBe('docker');
    expect(composer.request()).toMatchObject({
      agent: recent.agent,
      env: 'local',
      options: recent.options,
      ports: [],
    });
  });

  it('adopts a recent session as a new one', () => {
    const { composer, snapshot } = loaded();
    const recent = snapshot.recents[2]!;
    composer.session = 'continue';
    composer.adopt(recent);
    expect(composer.request()).toEqual({
      agent: recent.agent,
      env: recent.env,
      folder: recent.folder,
      options: recent.options,
      session: 'new',
      ports: ['127.0.0.1:3000:3000', '127.0.0.1:5173:5173'],
    });
    expect(recent.ports).toEqual(composer.request().ports);
  });

  it('adopts a Lokal recent with the default ports for a later Docker start', () => {
    const { composer, snapshot } = loaded();
    const settings = structuredClone(snapshot.settings);
    settings.docker.ports = ['127.0.0.1:8080:8080'];
    composer.sync({ ...snapshot, settings });
    const recent = {
      ...snapshot.recents[0]!,
      env: 'local' as const,
      ports: [],
    };
    composer.ports = ['127.0.0.1:1:1'];
    composer.adopt(recent);
    expect(composer.request().ports).toEqual([]);
    composer.selectEnv('docker');
    expect(composer.request().ports).toEqual(['127.0.0.1:8080:8080']);
    // A copy: the form never edits the saved defaults.
    composer.ports.push('127.0.0.1:1:1');
    expect(settings.docker.ports).toEqual(['127.0.0.1:8080:8080']);
  });

  it('trims the folder it sends', () => {
    const { composer } = loaded();
    composer.folder = '  C:\\Users\\demo\\x  ';
    expect(composer.request().folder).toBe('C:\\Users\\demo\\x');
  });

  it('sends no effort the model cannot carry (OpenCode without a model)', () => {
    const { composer } = loaded();
    composer.selectAgent('opencode');
    composer.options.effort = 'high';
    expect(composer.request().options.effort).toBe('default');
    // The choice is kept for when a model is typed.
    expect(composer.options.effort).toBe('high');
    composer.options.model = 'opencode/big-pickle';
    expect(composer.request().options).toMatchObject({
      model: 'opencode/big-pickle',
      effort: 'high',
    });
  });

  it('sends ports with a Docker start only and resets them after it', () => {
    const { composer, snapshot } = loaded();
    expect(composer.ports).toEqual([]);
    composer.ports = ['127.0.0.1:3000:3000'];
    expect(composer.request().ports).toEqual([]);
    composer.selectEnv('docker');
    const request = composer.request();
    expect(request.ports).toEqual(['127.0.0.1:3000:3000']);
    // A plain copy, detached from the form.
    composer.ports.push('127.0.0.1:5173:5173');
    expect(request.ports).toEqual(['127.0.0.1:3000:3000']);
    // An agent change keeps them; a start goes back to the defaults.
    composer.selectAgent('codex');
    expect(composer.ports).toHaveLength(2);
    composer.launched();
    expect(composer.ports).toEqual(snapshot.settings.docker.ports);
  });

  it('starts from the saved default ports and follows a saved change', () => {
    const { composer, snapshot } = loaded();
    const settings = structuredClone(snapshot.settings);
    settings.docker.ports = ['127.0.0.1:8080:8080'];
    composer.sync({ ...snapshot, settings });
    expect(composer.ports).toEqual(['127.0.0.1:8080:8080']);
    // An unrelated snapshot keeps the per-start choice.
    composer.ports = [];
    composer.sync({ ...snapshot, settings, revision: 9 });
    expect(composer.ports).toEqual([]);
    const fresh = new Composer();
    fresh.sync({ ...snapshot, settings });
    expect(fresh.ports).toEqual(['127.0.0.1:8080:8080']);
    // The form holds its own copy, never the snapshot's array.
    fresh.ports.push('127.0.0.1:1:1');
    expect(settings.docker.ports).toEqual(['127.0.0.1:8080:8080']);
  });

  it('sends a repository with an empty folder, a folder as typed', () => {
    const { composer } = loaded();
    composer.folder = ' https://github.com/demo-org/kunden-portal.git ';
    expect(composer.target).toEqual({
      kind: 'repo',
      repo: { owner: 'demo-org', name: 'kunden-portal' },
    });
    const request = composer.request();
    expect(request.folder).toBe('');
    expect(request.repo).toEqual({ owner: 'demo-org', name: 'kunden-portal' });
    composer.folder = 'C:\\Users\\demo\\x';
    expect(composer.request()).not.toHaveProperty('repo');
    expect(composer.request().folder).toBe('C:\\Users\\demo\\x');
    // Search text is neither: the composer blocks the start on it.
    composer.folder = 'tray agent';
    expect(composer.target).toEqual({ kind: 'search', query: 'tray agent' });
    expect(composer.request()).not.toHaveProperty('repo');
  });

  it('puts owner/name of a repository recent into the field', () => {
    const { composer, snapshot } = loaded();
    const recent = snapshot.recents.find((item) => item.repo)!;
    expect(recentTarget(recent)).toBe('demo-user/repo-watch');
    composer.adopt(recent);
    expect(composer.folder).toBe('demo-user/repo-watch');
    expect(composer.request()).toMatchObject({
      folder: '',
      repo: recent.repo,
      agent: recent.agent,
    });
    expect(recentTarget(snapshot.recents[0])).toBe(snapshot.recents[0]!.folder);
    expect(recentTarget(undefined)).toBe('');
    // No recent seeds the field (owner request, #20): it opens empty.
    const fresh = new Composer();
    fresh.sync({ ...snapshot, recents: [recent] });
    expect(fresh.folder).toBe('');
  });

  it('starts from the native default permission before any snapshot', () => {
    expect(new Composer().options).toEqual({
      model: 'default',
      effort: 'default',
      permission: 'auto',
      extraArgs: '',
    });
  });
});
