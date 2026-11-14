import { describe, expect, it, vi } from 'vitest';
import { demoSnapshot } from './demo';
import { DEBUG_HINT, ProfileActions } from './profileActions.svelte';
import type { Snapshot } from './types';

/** A sign-in the test ends by hand, against a labelled stand-in bridge. */
function setup(debug = false) {
  const shown: Snapshot[] = [];
  let current: Snapshot = demoSnapshot(0);
  current.settings.debug = debug;
  let finish: (value: Snapshot) => void = () => {};
  let fail: (cause: unknown) => void = () => {};
  const profileLogin = vi.fn(
    () =>
      new Promise<Snapshot>((resolve, reject) => {
        finish = resolve;
        fail = reject;
      }),
  );
  const cancelProfileLogin = vi.fn(async () => {
    fail('Anmeldung abgebrochen.');
  });
  const actions = new ProfileActions({
    bridge: {
      syncProfiles: async () => current,
      profileLogin,
      cancelProfileLogin,
    },
    accept: (value) => {
      current = value;
      shown.push(value);
      return true;
    },
    saveSettings: async () => current,
    current: () => current,
  });
  return {
    actions,
    shown,
    profileLogin,
    cancelProfileLogin,
    finish: (value: Snapshot) => finish(value),
    fail: (cause: unknown) => fail(cause),
  };
}

describe('ProfileActions sign-in (#41)', () => {
  it('runs a hidden sign-in, shows the answer and frees the row', async () => {
    const { actions, shown, finish } = setup();
    const result = actions.login('claude', 'team');
    expect(actions.running('claude', 'team')).toBe(true);
    expect(actions.running('claude', 'default')).toBe(false);
    // A second click on the running row starts nothing.
    expect(await actions.login('claude', 'team')).toBe('failed');
    const signed = { ...demoSnapshot(0), revision: 5 };
    finish(signed);
    expect(await result).toBe('signedIn');
    expect(shown).toEqual([signed]);
    expect(actions.running('claude', 'team')).toBe(false);
    expect(actions.failure).toBeNull();
  });

  it('keeps a failure with the „Debug“ hint until the next action', async () => {
    const { actions, fail } = setup();
    const result = actions.login('claude', 'team');
    fail('Anmeldung abgelaufen.');
    expect(await result).toBe('failed');
    expect(actions.failure).toEqual({
      agent: 'claude',
      text: 'Anmeldung abgelaufen.',
      hint: true,
    });
    expect(DEBUG_HINT).toBe('Mit Debug erneut versuchen');
    expect(actions.running('claude', 'team')).toBe(false);
    await actions.sync();
    expect(actions.failure).toBeNull();
  });

  it('a cancel ends the sign-in quietly', async () => {
    const { actions, cancelProfileLogin } = setup();
    const result = actions.login('claude', 'team');
    await actions.cancel('claude', 'team');
    expect(cancelProfileLogin).toHaveBeenCalledWith('claude', 'team');
    expect(await result).toBe('cancelled');
    expect(actions.failure).toBeNull();
    expect(actions.running('claude', 'team')).toBe(false);
    // Nothing runs: a cancel calls nothing.
    await actions.cancel('claude', 'team');
    expect(cancelProfileLogin).toHaveBeenCalledTimes(1);
  });

  it('with „Debug“ the terminal opened: no running state, no hint', async () => {
    const { actions, finish, fail } = setup(true);
    const opened = actions.login('claude', 'team');
    expect(actions.running('claude', 'team')).toBe(false);
    const answer = demoSnapshot(0);
    answer.settings.debug = true;
    finish(answer);
    expect(await opened).toBe('opened');
    const refused = actions.login('claude', 'team');
    fail('Claude Code ist nicht installiert.');
    expect(await refused).toBe('failed');
    expect(actions.failure?.hint).toBe(false);
  });
});
