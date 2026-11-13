import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { demoSettings } from './demoData';
import {
  HANDLED_NAMES,
  MAX_SHARED,
  NEVER_SHARED,
  PROTECTED_PREFIXES,
  agentShare,
  parseShareNames,
  shareProblems,
  shareText,
} from './profileShare';
import { settingsProblems } from './validation';

const CC = 'Claude Code';
const problems = (dirs: string[], files: string[] = []) =>
  shareProblems({ dirs, files }, CC);
const names = (count: number, stem: string) =>
  Array.from({ length: count }, (_, index) => `${stem}${index}`);

describe('shared names: the fields', () => {
  it('split at commas and whitespace, keep the order, drop exact repeats', () => {
    expect(parseShareNames('')).toEqual([]);
    expect(parseShareNames('  ,  ')).toEqual([]);
    expect(parseShareNames('a, b  c,d\te\n,a')).toEqual([
      'a',
      'b',
      'c',
      'd',
      'e',
    ]);
    // Only exact repeats go; a different case stays for the duplicate check.
    expect(parseShareNames('notes Notes')).toEqual(['notes', 'Notes']);
    expect(shareText(['.caveman-sessions', 'x'])).toBe('.caveman-sessions, x');
  });

  it('read an agent without a share as empty', () => {
    const settings = demoSettings();
    expect(agentShare(settings, 'claude')).toEqual({
      dirs: ['.caveman-sessions'],
      files: ['orca-state.json'],
    });
    expect(agentShare({ profileShare: undefined! }, 'codex')).toEqual({
      dirs: [],
      files: [],
    });
  });
});

describe('shared names: the rules', () => {
  it('accept plain names, dot files and names that only look like built-ins', () => {
    expect(
      problems(
        ['.caveman-sessions', 'a', 'a'.repeat(64), 'skills2', 'CON2'],
        ['orca-state.json', 'settings.local.json', 'nul_x', 'claude.json'],
      ),
    ).toEqual({});
  });

  it('refuse what is no plain file or folder name', () => {
    for (const name of [
      '.',
      '..',
      'a.',
      'a'.repeat(65),
      'a b',
      'a/b',
      'a\\b',
      'ä',
      'x:y',
      '*',
    ])
      expect(problems([name]).dirs, name).toBe(
        `Name „${name}“ für ${CC} ist ungültig.`,
      );
  });

  it('refuse Windows device names, with or without an extension, in any case', () => {
    for (const name of [
      'CON',
      'prn',
      'Aux',
      'nul.txt',
      'COM1',
      'com9.log',
      'LPT1',
      'lpt9',
    ])
      expect(problems([], [name]).files, name).toBe(
        `Name „${name}“ für ${CC} ist ungültig.`,
      );
    expect(problems(['COM10', 'LPT', 'connect'])).toEqual({});
  });

  it('refuse Traygent’s own names, in any case', () => {
    for (const name of ['.traygent', '.traygent-sync.json', '.Traygent2'])
      expect(problems([name]).dirs, name).toBe(
        `Name „${name}“ für ${CC} ist ungültig.`,
      );
  });

  it('refuse what the sync already handles, in any case and in either list', () => {
    for (const name of [
      'skills',
      'paste-cache',
      'hooks',
      'todos',
      'session-env',
      'Plugins',
    ])
      expect(problems([name]).dirs, name).toBe(
        `Name „${name}“ für ${CC} wird bereits abgeglichen.`,
      );
    for (const name of [
      'settings.json',
      'claude.md',
      'keybindings.json',
      'history.jsonl',
    ])
      expect(problems([], [name]).files, name).toBe(
        `Name „${name}“ für ${CC} wird bereits abgeglichen.`,
      );
    // A built-in directory name in the files list is handled all the same.
    expect(problems([], ['projects']).files).toBe(
      `Name „projects“ für ${CC} wird bereits abgeglichen.`,
    );
  });

  it('refuse the state and sign-in files, their copies and the account caches', () => {
    for (const name of [
      '.claude.json',
      '.claude.json.backup',
      '.claude.jsonx',
      '.CREDENTIALS.json',
      '.credentials.json.bak',
      'remote-settings.json',
      'Policy-Limits.json',
    ])
      expect(problems([], [name]).files, name).toBe(
        `Name „${name}“ für ${CC} ist ungültig.`,
      );
    for (const name of ['backups', 'statsig', 'Cache', 'telemetry', 'debug'])
      expect(problems([name]).dirs, name).toBe(
        `Name „${name}“ für ${CC} ist ungültig.`,
      );
    expect(
      problems(['caches', 'my-debug'], ['remote-settings.json.x']),
    ).toEqual({});
  });

  it('hold the same name lists as the native side', () => {
    const native = JSON.parse(
      readFileSync(
        join(process.cwd(), 'fixtures/profile-share-names.json'),
        'utf8',
      ),
    ).claude;
    const lower = (list: string[]) =>
      list.map((name) => name.toLowerCase()).sort();
    expect([...HANDLED_NAMES].sort()).toEqual(lower(native.handled));
    expect([...NEVER_SHARED].sort()).toEqual(lower(native.neverShared));
    expect([...PROTECTED_PREFIXES].sort()).toEqual(
      lower(native.protectedPrefixes),
    );
    expect(native.ownPrefix).toBe('.traygent');
    expect(native.maxPerList).toBe(MAX_SHARED);
    expect(native.maxNameChars).toBe(64);
  });

  it('refuse a name twice, ignoring case, within and across both lists', () => {
    expect(problems(['notes', 'Notes'])).toEqual({
      dirs: `Name „Notes“ für ${CC} ist doppelt vergeben.`,
    });
    // Across the lists the file is the second one.
    expect(problems(['state.json'], ['STATE.json'])).toEqual({
      files: `Name „STATE.json“ für ${CC} ist doppelt vergeben.`,
    });
  });

  it('allow 16 per list, not 17', () => {
    expect(problems(names(MAX_SHARED, 'd'), names(MAX_SHARED, 'f'))).toEqual(
      {},
    );
    expect(problems(names(17, 'd'), names(17, 'f'))).toEqual({
      dirs: `Höchstens 16 weitere Ordner für ${CC} möglich.`,
      files: `Höchstens 16 weitere Dateien für ${CC} möglich.`,
    });
  });

  it('give each list its first problem only', () => {
    expect(problems(['a b', 'skills'], ['ok', 'nul'])).toEqual({
      dirs: `Name „a b“ für ${CC} ist ungültig.`,
      files: `Name „nul“ für ${CC} ist ungültig.`,
    });
  });

  it('reach the settings form per list, named with the agent’s label', () => {
    const settings = demoSettings();
    expect(settingsProblems(settings)).toEqual({});
    settings.profileShare.claude = { dirs: ['skills'], files: ['a b'] };
    expect(settingsProblems(settings, { claude: CC })).toEqual({
      'agents.claude.share.dirs': `Name „skills“ für ${CC} wird bereits abgeglichen.`,
      'agents.claude.share.files': `Name „a b“ für ${CC} ist ungültig.`,
    });
    // Without a label the id stands in.
    expect(settingsProblems(settings)['agents.claude.share.dirs']).toBe(
      'Name „skills“ für claude wird bereits abgeglichen.',
    );
  });
});
