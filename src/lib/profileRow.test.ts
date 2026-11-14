import { describe, expect, it } from 'vitest';
import { SIGNED_OUT, rowAccount } from './profileRow';
import type { ProfileStatus } from './types';

const status = (
  id: string,
  patch: Partial<ProfileStatus> = {},
): ProfileStatus => ({
  agent: 'claude',
  id,
  label: id,
  dir: '',
  signedIn: true,
  account: 'max@example.com',
  ...patch,
});

describe('rowAccount', () => {
  it('shows the account, or that the profile is signed out', () => {
    const solo = status('default', { organization: 'Beispiel GmbH' });
    expect(rowAccount(solo, [solo])).toEqual({
      text: 'max@example.com',
      title: 'max@example.com · Beispiel GmbH',
      out: false,
    });
    const out = status('team', { signedIn: false });
    expect(rowAccount(out, [solo, out])).toEqual({
      text: SIGNED_OUT,
      title: 'Nicht angemeldet',
      out: true,
    });
    expect(rowAccount(undefined, [])).toMatchObject({ out: true });
    const unnamed = status('x');
    delete unnamed.account;
    expect(rowAccount(unnamed, [unnamed]).text).toBe('Angemeldet');
  });

  it('tells rows with the same account apart by their organization', () => {
    const a = status('default', { organization: 'Beispiel GmbH' });
    const b = status('team', {
      account: 'MAX@example.com',
      organization: 'Beispiel Team',
    });
    expect(rowAccount(a, [a, b])).toEqual({
      text: 'Beispiel GmbH',
      title: 'max@example.com · Beispiel GmbH',
      out: false,
    });
    expect(rowAccount(b, [a, b]).text).toBe('Beispiel Team');
    // Without an organization nothing changes.
    const plain = status('solo');
    expect(rowAccount(plain, [a, plain]).text).toBe('max@example.com');
    expect(rowAccount(a, [a, plain]).text).toBe('Beispiel GmbH');
    // Nothing differs: the same organization keeps the account.
    const twin = status('twin', { organization: 'Beispiel GmbH' });
    expect(rowAccount(a, [a, twin]).text).toBe('max@example.com');
    // A signed-out profile, another agent or another account is no twin.
    const others = [
      status('out', { signedIn: false, organization: 'X' }),
      status('codex', { agent: 'codex', organization: 'Y' }),
      status('other', { account: 'eva@example.com', organization: 'Z' }),
    ];
    expect(rowAccount(a, [a, ...others]).text).toBe('max@example.com');
  });
});
