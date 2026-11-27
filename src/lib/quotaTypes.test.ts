import { describe, expect, it } from 'vitest';
import {
  MAX_QUOTA_PROFILES,
  quotaReadTip,
  shownQuotaProfiles,
} from './quotaTypes';
import type { AppSettings } from './types';

const known = ['default', 'team', 'work', 'side'];
const choice = (claude?: string[]): Pick<AppSettings, 'quotaProfiles'> => ({
  quotaProfiles: claude ? { claude } : {},
});

describe('shownQuotaProfiles', () => {
  it('shows only the start profile without a saved choice', () => {
    expect(shownQuotaProfiles({}, 'claude', 'team', known)).toEqual(['team']);
    expect(
      shownQuotaProfiles(choice(undefined), 'claude', 'default', known),
    ).toEqual(['default']);
    expect(shownQuotaProfiles(choice([]), 'claude', 'work', known)).toEqual([
      'work',
    ]);
  });

  it('always adds the start profile and keeps the order of the known list', () => {
    expect(
      shownQuotaProfiles(choice(['work', 'default']), 'claude', 'team', known),
    ).toEqual(['default', 'team', 'work']);
  });

  it('drops ids the agent does not know and duplicates', () => {
    expect(
      shownQuotaProfiles(
        choice(['gone', 'team', 'team', 'default']),
        'claude',
        'default',
        known,
      ),
    ).toEqual(['default', 'team']);
  });

  it('keeps the start profile and the earliest others past the cap', () => {
    expect(MAX_QUOTA_PROFILES).toBe(3);
    expect(
      shownQuotaProfiles(
        choice(['default', 'team', 'work', 'side']),
        'claude',
        'side',
        known,
      ),
    ).toEqual(['default', 'team', 'side']);
    expect(
      shownQuotaProfiles(
        choice(['default', 'team', 'work', 'side']),
        'claude',
        'default',
        known,
      ),
    ).toEqual(['default', 'team', 'work']);
  });

  it('reads only the agent asked for', () => {
    const settings = { quotaProfiles: { codex: ['team'] } };
    expect(shownQuotaProfiles(settings, 'claude', 'default', known)).toEqual([
      'default',
    ]);
  });

  it('keeps a start profile the known list misses, first', () => {
    expect(
      shownQuotaProfiles(choice(['team']), 'claude', 'new', known),
    ).toEqual(['new', 'team']);
  });
});

describe('quotaReadTip', () => {
  it('names what each read uses', () => {
    expect(quotaReadTip('claude')).toBe(
      'Startet die Claude-Code-CLI (/usage) mit der Anmeldung dieses Profils',
    );
    expect(quotaReadTip('codex')).toMatch(
      /CLI-Anmeldung.*nicht offiziell dokumentierte Schnittstelle/,
    );
    expect(quotaReadTip('opencode')).toMatch(/OpenCode-Go-Schlüssel/);
  });
});
