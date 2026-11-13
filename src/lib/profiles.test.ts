import { describe, expect, it } from 'vitest';
import { demoSnapshot } from './demoData';
import {
  editedProfile,
  recentProfileItems,
  MAX_PROFILES,
  agentProfiles,
  defaultProfile,
  launchProfileProblem,
  newProfileProblem,
  profileFolder,
  profileId,
  profileInitial,
  profileLabel,
  profilesProblem,
  renameProblem,
} from './profiles';
import type { AgentProfile } from './types';
import { settingsProblems } from './validation';

const team: AgentProfile = { id: 'team', label: 'Team' };

describe('profile ids', () => {
  it('derive from the typed name: lower case, one - per run, trimmed, 24 at most', () => {
    expect(profileId('Team')).toBe('team');
    expect(profileId('  Kunde A / Projekt  ')).toBe('kunde-a-projekt');
    expect(profileId('--Max__Muster--')).toBe('max-muster');
    expect(profileId('Büro 2')).toBe('buero-2');
    expect(profileId('a'.repeat(30))).toBe('a'.repeat(24));
    // A cut never leaves a trailing -.
    expect(profileId(`${'a'.repeat(23)} b`)).toBe('a'.repeat(23));
    expect(profileId('日本')).toBe('');
  });

  it('spell out ä ö ü ß and drop other diacritics before the slug', () => {
    expect(profileId('Büro')).toBe('buero');
    expect(profileId('ÄRGER Öl Übung')).toBe('aerger-oel-uebung');
    expect(profileId('Straße')).toBe('strasse');
    expect(profileId('Café Señor')).toBe('cafe-senor');
    // A decomposed ü (u + combining diaeresis) is the same letter.
    expect(profileId('Bu\u0308ro')).toBe('buero');
  });

  it('name the folder Rust derives, for the tooltip', () => {
    expect(profileFolder('claude', 'team')).toBe('%USERPROFILE%\\.claude-team');
    expect(profileFolder('claude', 'default')).toBe('%USERPROFILE%\\.claude');
  });

  it('mark a profile by its initial', () => {
    expect(profileInitial('team')).toBe('T');
    expect(profileInitial(' Ökologie')).toBe('Ö');
  });
});

describe('a new profile', () => {
  it('needs a name of 1–24 characters with a letter or digit', () => {
    expect(newProfileProblem('Team', [])).toBe('');
    expect(newProfileProblem('   ', [])).toBe('Name fehlt.');
    expect(newProfileProblem('x'.repeat(25), [])).toBe('Höchstens 24 Zeichen.');
    expect(newProfileProblem('a\tb', [])).toBe('Keine Steuerzeichen.');
    expect(newProfileProblem('日本', [])).toBe(
      'Mindestens ein Buchstabe oder eine Ziffer.',
    );
  });

  it('never takes „Standard“, `default` or a name or id already there', () => {
    expect(newProfileProblem('standard', [])).toBe(
      '„standard“ ist reserviert.',
    );
    expect(newProfileProblem('Default', [])).toBe('„Default“ ist reserviert.');
    expect(newProfileProblem('TEAM', [team])).toBe('Profil gibt es schon.');
    // Another label with the same id would share its folder.
    expect(newProfileProblem('Team!', [team])).toBe('Profil gibt es schon.');
  });

  it(`stops at ${MAX_PROFILES} per agent`, () => {
    const full = Array.from({ length: MAX_PROFILES }, (_, i) => ({
      id: `p${i}`,
      label: `P${i}`,
    }));
    expect(newProfileProblem('Neu', full)).toBe('Höchstens 8 Profile.');
  });
});

describe('a saved profile list', () => {
  it('passes valid lists and a default that is listed', () => {
    expect(profilesProblem([], 'default')).toBe('');
    expect(profilesProblem([team], 'team')).toBe('');
    // A saved id need not be derived from its label.
    expect(profilesProblem([{ id: 'x1', label: 'Kunde' }], 'default')).toBe('');
  });

  it('refuses what Rust refuses, naming the profile', () => {
    expect(profilesProblem([team], 'gone')).toBe(
      'Standard-Profil gibt es nicht.',
    );
    expect(profilesProblem([{ id: 'Team', label: 'Team' }], 'default')).toBe(
      'Team: Ungültige Kennung.',
    );
    expect(profilesProblem([{ id: 'default', label: 'X' }], 'default')).toBe(
      'X: Ungültige Kennung.',
    );
    expect(
      profilesProblem([team, { id: 't2', label: 'team' }], 'default'),
    ).toBe('team: Profil gibt es schon.');
    expect(profilesProblem([{ id: 's', label: 'Standard' }], 'default')).toBe(
      'Standard: „Standard“ ist reserviert.',
    );
    expect(profilesProblem([{ id: 's', label: ' S' }], 'default')).toBe(
      ' S: Ungültiger Name.',
    );
  });

  it('checks a rename in place by the same rules, unprefixed', () => {
    const list = [team, { id: 'kunde', label: 'Kunde' }];
    expect(renameProblem(list, 0)).toBe('');
    expect(renameProblem([{ id: 'team', label: 'Kunde B' }], 0)).toBe('');
    expect(renameProblem([{ id: 'team', label: '' }], 0)).toBe('Name fehlt.');
    expect(renameProblem([{ id: 'team', label: 'STANDARD' }], 0)).toBe(
      '„STANDARD“ ist reserviert.',
    );
    expect(renameProblem([team, { id: 'kunde', label: 'TEAM' }], 1)).toBe(
      'Profil gibt es schon.',
    );
    expect(renameProblem(list, 5)).toBe('');
  });

  it('marks the field in the settings problems', () => {
    const settings = demoSnapshot().settings;
    expect(settingsProblems(settings)).toEqual({});
    settings.agentProfile.claude = 'gone';
    expect(settingsProblems(settings)).toEqual({
      'agents.claude.profiles': 'Standard-Profil gibt es nicht.',
    });
  });
});

describe('profiles of a snapshot', () => {
  it('list „Standard“ first and fall back to the saved list', () => {
    const snapshot = demoSnapshot();
    expect(agentProfiles(snapshot, 'claude').map((p) => p.label)).toEqual([
      'Standard',
      'Team',
    ]);
    expect(agentProfiles({ ...snapshot, profiles: [] }, 'claude')).toEqual([
      {
        agent: 'claude',
        id: 'default',
        label: 'Standard',
        dir: '',
        signedIn: false,
      },
      { agent: 'claude', id: 'team', label: 'Team', dir: '', signedIn: false },
    ]);
    expect(profileLabel(snapshot, 'claude', 'team')).toBe('Team');
    expect(profileLabel(snapshot, 'claude', undefined)).toBe('Standard');
    expect(profileLabel(snapshot, 'claude', 'gone')).toBe('gone');
  });

  it('start from the saved default, „Standard“ when it is gone', () => {
    const { settings } = demoSnapshot();
    expect(defaultProfile(settings, 'claude')).toBe('default');
    settings.agentProfile.claude = 'team';
    expect(defaultProfile(settings, 'claude')).toBe('team');
    settings.profiles.claude = [];
    expect(defaultProfile(settings, 'claude')).toBe('default');
  });

  it('refuse a start with a profile that is gone, outside Lokal or the agent', () => {
    const snapshot = demoSnapshot();
    const start = { agent: 'claude' as const, env: 'local' as const };
    expect(launchProfileProblem(snapshot, start)).toBe('');
    expect(launchProfileProblem(snapshot, { ...start, profile: 'team' })).toBe(
      '',
    );
    expect(launchProfileProblem(snapshot, { ...start, profile: 'alt' })).toBe(
      'Profil „alt“ gibt es nicht mehr.',
    );
    expect(
      launchProfileProblem(snapshot, {
        ...start,
        env: 'docker',
        profile: 'team',
      }),
    ).toBe('Profile gelten nur lokal.');
    expect(
      launchProfileProblem(snapshot, {
        ...start,
        agent: 'codex',
        profile: 'team',
      }),
    ).toBe('Dieser Agent hat keine Profile.');
  });
});

describe('the profile of an edited recent', () => {
  it('is offered for a Lokal entry of an agent with an extra profile', () => {
    const snapshot = demoSnapshot();
    const items = (
      agent: 'claude' | 'codex',
      env: 'local' | 'docker',
      stored?: string,
    ) => recentProfileItems(snapshot, agent, env, stored);
    expect(items('claude', 'local')).toEqual([
      { id: 'default', label: 'Standard' },
      { id: 'team', label: 'Team' },
    ]);
    expect(items('claude', 'docker', 'team')).toEqual([]);
    expect(items('codex', 'local')).toEqual([]);
    snapshot.settings.profiles.claude = [];
    snapshot.profiles = [];
    expect(items('claude', 'local')).toEqual([]);
    expect(items('claude', 'local', 'default')).toEqual([]);
    // A profile the entry still stores keeps the field, with „Standard“ only.
    expect(items('claude', 'local', 'team')).toEqual([
      { id: 'default', label: 'Standard' },
    ]);
  });

  it('is stored as natively: none for „Standard“ and outside Lokal', () => {
    const { settings } = demoSnapshot();
    expect(editedProfile(settings, 'claude', 'local', ' team ')).toBe('team');
    for (const standard of [undefined, '', '  ', 'default'])
      expect(
        editedProfile(settings, 'claude', 'local', standard),
      ).toBeUndefined();
    expect(editedProfile(settings, 'claude', 'docker', 'team')).toBeUndefined();
    expect(editedProfile(settings, 'claude', 'local', 'gone')).toBeNull();
    expect(editedProfile(settings, 'codex', 'local', 'team')).toBeNull();
  });
});
