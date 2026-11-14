import { describe, expect, it, vi } from 'vitest';
import { demoSnapshot } from './demo';
import { signedIn } from './demoProfiles';
import { DEBUG_HINT, ProfileActions } from './profileActions.svelte';
import type { Snapshot } from './types';

/** A sign-in the test ends by hand, against a labelled stand-in bridge. */
function setup(debug = false, found = true) {
  const shown: Snapshot[] = [];
  const base = (signed: boolean) => {
    const value = demoSnapshot(0);
    value.settings.debug = debug;
    if (signed) value.profiles = signedIn(value.profiles, 'claude', 'team');
    return value;
  };
  let current = base(false);
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
    if (found) fail('Anmeldung abgebrochen.');
    return found;
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
    base,
    profileLogin,
    cancelProfileLogin,
    finish: (value: Snapshot) => finish(value),
    fail: (cause: unknown) => fail(cause),
  };
}

describe('ProfileActions sign-in (#41)', () => {
  it('runs a hidden sign-in, shows the answer and frees the row', async () => {
    const { actions, shown, base, finish, profileLogin } = setup();
    const result = actions.login('claude', 'team');
    expect(actions.running('claude', 'team')).toBe(true);
    expect(actions.running('claude', 'default')).toBe(false);
    // A second click on the pending row starts nothing.
    expect(await actions.login('claude', 'team')).toBe('failed');
    expect(profileLogin).toHaveBeenCalledTimes(1);
    const signed = base(true);
    finish(signed);
    expect(await result).toBe('signedIn');
    expect(shown).toEqual([signed]);
    expect(actions.running('claude', 'team')).toBe(false);
    expect(actions.failure).toBeNull();
  });

  it('an answer that still reads signed out is a failed sign-in', async () => {
    const { actions, base, finish } = setup();
    const result = actions.login('claude', 'team');
    finish(base(false));
    expect(await result).toBe('failed');
    expect(actions.failure).toEqual({
      agent: 'claude',
      profile: 'team',
      text: 'Anmeldung fehlgeschlagen.',
      hint: true,
    });
  });

  it('keeps a failure with the „Debug“ hint until that profile starts again', async () => {
    const { actions, base, finish, fail } = setup();
    const result = actions.login('claude', 'team');
    fail('Anmeldung abgelaufen.');
    expect(await result).toBe('failed');
    expect(actions.failure).toEqual({
      agent: 'claude',
      profile: 'team',
      text: 'Anmeldung abgelaufen.',
      hint: true,
    });
    expect(DEBUG_HINT).toBe('Mit Debug erneut versuchen');
    expect(actions.running('claude', 'team')).toBe(false);
    // Another profile's sign-in keeps it; that profile's next one clears it.
    void actions.login('claude', 'default');
    expect(actions.failure?.profile).toBe('team');
    finish(base(true));
    const again = actions.login('claude', 'team');
    expect(actions.failure).toBeNull();
    finish(base(true));
    expect(await again).toBe('signedIn');
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
    // Nothing pending: a cancel calls nothing.
    await actions.cancel('claude', 'team');
    expect(cancelProfileLogin).toHaveBeenCalledTimes(1);
  });

  it('a cancel that found nothing does not hide a later real failure', async () => {
    const { actions, fail } = setup(false, false);
    const result = actions.login('claude', 'team');
    await actions.cancel('claude', 'team');
    fail('Anmeldung fehlgeschlagen.');
    expect(await result).toBe('failed');
    expect(actions.failure?.text).toBe('Anmeldung fehlgeschlagen.');
  });

  it('with „Debug“ the row waits until the terminal opened, no hint after', async () => {
    const { actions, base, finish, fail, profileLogin } = setup(true);
    const opened = actions.login('claude', 'team');
    expect(actions.running('claude', 'team')).toBe(true);
    expect(await actions.login('claude', 'team')).toBe('failed');
    expect(profileLogin).toHaveBeenCalledTimes(1);
    finish(base(false));
    expect(await opened).toBe('opened');
    expect(actions.running('claude', 'team')).toBe(false);
    const refused = actions.login('claude', 'team');
    fail('Claude Code ist nicht installiert.');
    expect(await refused).toBe('failed');
    expect(actions.failure?.hint).toBe(false);
  });
});
