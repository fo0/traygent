import { describe, expect, it } from 'vitest';
import { Composer } from './composerState.svelte';
import { demoSnapshot } from './demoData';
import type { RecentSession, Snapshot } from './types';

const loaded = (change: (snapshot: Snapshot) => void = () => {}) => {
  const composer = new Composer();
  const snapshot = demoSnapshot();
  change(snapshot);
  composer.sync(snapshot);
  return { composer, snapshot };
};
const ids = (composer: Composer) => composer.profiles.map((p) => p.id);

describe('Composer profile', () => {
  it('offers „Standard“ and the extra profiles for Claude Code in Lokal only', () => {
    const { composer } = loaded();
    expect(composer.profile).toBe('default');
    expect(ids(composer)).toEqual(['default', 'team']);
    composer.selectEnv('docker');
    expect(ids(composer)).toEqual([]);
    composer.selectEnv('local');
    composer.selectAgent('codex');
    expect(ids(composer)).toEqual([]);
  });

  it('shows no chip without an extra profile', () => {
    const { composer } = loaded((snapshot) => {
      snapshot.settings.profiles.claude = [];
      snapshot.profiles = snapshot.profiles.filter((p) => p.id === 'default');
    });
    expect(ids(composer)).toEqual([]);
    expect(composer.request()).not.toHaveProperty('profile');
  });

  it('starts from the saved default and changes the next start only', () => {
    const { composer, snapshot } = loaded((value) => {
      value.settings.agentProfile.claude = 'team';
    });
    expect(composer.profile).toBe('team');
    expect(composer.request().profile).toBe('team');
    composer.profile = 'default';
    // `default` is never sent.
    expect(composer.request()).not.toHaveProperty('profile');
    composer.profile = 'team';
    composer.launched();
    expect(composer.profile).toBe('team');
    expect(snapshot.settings.agentProfile.claude).toBe('team');
    composer.profile = 'default';
    composer.selectAgent('codex');
    composer.selectAgent('claude');
    expect(composer.profile).toBe('team');
  });

  it('sends no profile from Docker', () => {
    const { composer } = loaded();
    composer.profile = 'team';
    composer.selectEnv('docker');
    expect(composer.request()).not.toHaveProperty('profile');
  });

  it('follows a saved default and a removed profile, keeping other choices', () => {
    const { composer, snapshot } = loaded();
    composer.profile = 'team';
    composer.sync({ ...snapshot, revision: 2 });
    expect(composer.profile).toBe('team');
    const removed = structuredClone(snapshot);
    removed.settings.profiles.claude = [];
    composer.sync(removed);
    expect(composer.profile).toBe('default');
    const chosen = structuredClone(snapshot);
    chosen.settings.agentProfile.claude = 'team';
    composer.sync(chosen);
    expect(composer.profile).toBe('team');
  });

  it('takes a recent’s profile over, else the default when it is gone', () => {
    const { composer, snapshot } = loaded((value) => {
      value.settings.agentProfile.claude = 'team';
    });
    const recent = snapshot.recents.find((r) => r.profile === 'team');
    expect(recent).toBeDefined();
    const standard: RecentSession = { ...recent! };
    delete standard.profile;
    composer.adopt(standard);
    expect(composer.profile).toBe('default');
    composer.adopt(recent!);
    expect(composer.profile).toBe('team');
    expect(composer.request().profile).toBe('team');
    composer.adopt({ ...recent!, profile: 'weg' });
    expect(composer.profile).toBe('team');
    expect(composer.folder).toBe(recent!.folder);
  });
});
