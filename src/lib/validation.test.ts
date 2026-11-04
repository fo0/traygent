import { describe, expect, it } from 'vitest';
import { demoSettings } from './demoData';
import {
  argsProblem,
  folderProblem,
  imageProblem,
  launchPortsProblem,
  modelProblem,
  optionsProblem,
  settingsProblems,
  shimArgsProblem,
} from './validation';

/*
 * The panel-side checks mirror src-tauri/src/settings.rs; each case below is
 * one of the native rules (see its tests in src-tauri/src/settings/tests.rs).
 */
describe('extra arguments', () => {
  it('accepts plain flags and paths', () => {
    expect(argsProblem('')).toBe('');
    expect(argsProblem('  --verbose   --add-dir  C:\\x  ')).toBe('');
    expect(argsProblem('a;b &c')).toBe('');
  });
  it('rejects ASCII, backtick and typographic quotes', () => {
    for (const raw of [
      '--x "y"',
      "it's",
      '`x',
      '\u2018x',
      '\u2019x',
      '\u201ax',
      '\u201bx',
      '\u201cx',
      '\u201dx',
      '\u201ex',
    ])
      expect(argsProblem(raw), raw).toBe('Keine Anführungszeichen.');
  });
  it('rejects control characters, also C1', () => {
    for (const raw of ['a\tb', 'a\nb', 'a\u0007', 'a\u007f', 'a\u0085'])
      expect(argsProblem(raw), JSON.stringify(raw)).toBe(
        'Keine Steuerzeichen.',
      );
  });
  it('rejects more than 1000 characters', () => {
    expect(argsProblem('a'.repeat(1000))).toBe('');
    expect(argsProblem('a'.repeat(1001))).toBe('Zu lang.');
    // Counted like Rust's `chars()`: one per Unicode scalar value.
    expect(argsProblem('😀'.repeat(1000))).toBe('');
  });
  it('rejects cmd.exe metacharacters only for an npm `.cmd` shim', () => {
    expect(shimArgsProblem('--ok a=b')).toBe('');
    for (const bad of ['a&b', 'a|b', '<', '>', '^', '%PATH%', '!x!', '(', 'a)'])
      expect(shimArgsProblem(bad), bad).not.toBe('');
  });
});

describe('model', () => {
  it('accepts aliases, ids and provider paths', () => {
    for (const model of [
      '',
      'default',
      'opus',
      'claude-opus-5-5',
      'opus[1m]',
      'us.anthropic.x:0',
      'gpt-6.1-sol',
      'my-custom@2',
    ])
      expect(modelProblem(model, 'claude'), model).toBe('');
  });
  it('rejects unsafe characters, like the native charset', () => {
    for (const model of [
      'opus sonnet',
      'a;b',
      "a'b",
      'a"b',
      'a&b',
      'a#b',
      'ü',
      'a`b',
      'a\u201cb',
    ])
      expect(modelProblem(model, 'claude'), model).toBe('Ungültiges Modell.');
  });
  it('rejects a leading `-`, which would become a flag', () => {
    expect(modelProblem('--dangerously-skip-permissions', 'claude')).toBe(
      'Ungültiges Modell.',
    );
    expect(modelProblem('-m', 'codex')).toBe('Ungültiges Modell.');
  });
  it('rejects more than 128 characters', () => {
    expect(modelProblem('a'.repeat(128), 'codex')).toBe('');
    expect(modelProblem('a'.repeat(129), 'codex')).toBe('Modell zu lang.');
  });
  it('needs OpenCode’s provider/model form', () => {
    for (const model of [
      'opencode/big-pickle',
      'github-copilot/claude-sonnet-5.5',
      'a.b/c:d@e/f',
    ])
      expect(modelProblem(model, 'opencode'), model).toBe('');
    for (const model of [
      'big-pickle',
      '/model',
      'provider/',
      'a:b/c',
      'a/b[1]',
    ])
      expect(modelProblem(model, 'opencode'), model).toBe(
        'Modell als provider/model angeben.',
      );
    // Other agents take a bare id.
    expect(modelProblem('big-pickle', 'claude')).toBe('');
  });
});

describe('Docker image and folder', () => {
  it('accepts Docker references', () => {
    for (const image of [
      'node:22-trixie',
      'ghcr.io/org/image:1.2',
      'registry:5000/a/b@sha256:abc',
    ])
      expect(imageProblem(image), image).toBe('');
  });
  it('rejects a blank, a leading non-alphanumeric and unsafe characters', () => {
    expect(imageProblem('  ')).toBe('Image fehlt.');
    for (const image of ['-x', '.x', 'node 22', 'node;rm', 'a"b', 'a\u2019b'])
      expect(imageProblem(image), image).toBe('Ungültiges Image.');
    expect(imageProblem(`a${'b'.repeat(255)}`)).toBe('Ungültiges Image.');
  });
  it('needs an absolute Windows folder', () => {
    expect(folderProblem('C:\\x')).toBe('');
    expect(folderProblem('\\\\server\\share')).toBe('');
    // Rust normalizes forward-slash UNC paths too.
    expect(folderProblem('//server/share')).toBe('');
    expect(folderProblem('\\\\')).toBe(
      'Ordner muss ein absoluter Windows-Pfad sein.',
    );
    expect(folderProblem('x\\y')).toBe(
      'Ordner muss ein absoluter Windows-Pfad sein.',
    );
    expect(folderProblem('C:\\a\u0000')).toBe('Keine Steuerzeichen.');
  });
});

describe('options and settings', () => {
  const options = { model: 'default', effort: 'default', extraArgs: '' };
  it('names the first problem of a start', () => {
    const permission = 'auto' as const;
    expect(
      optionsProblem({
        agent: 'opencode',
        options: { ...options, permission, model: 'x' },
      }),
    ).toBe('Modell als provider/model angeben.');
    expect(
      optionsProblem({
        agent: 'codex',
        options: { ...options, permission, extraArgs: '--x="y"' },
      }),
    ).toBe('Extra-Argumente: Keine Anführungszeichen.');
    const shimmed = {
      agent: 'codex' as const,
      options: { ...options, permission, extraArgs: '--x a&b' },
    };
    expect(optionsProblem(shimmed)).toBe('');
    expect(optionsProblem(shimmed, true)).toMatch(/^Extra-Argumente: /);
  });
  it('maps every invalid field of the settings', () => {
    const settings = demoSettings();
    expect(settingsProblems(settings)).toEqual({});
    settings.agents.opencode.model = 'big-pickle';
    settings.agents.claude.extraArgs = '\u201ex\u201c';
    settings.docker.images.codex = 'node 22';
    settings.docker.ports = ['127.0.0.1:3000:3000', '3000:80'];
    expect(settingsProblems(settings)).toEqual({
      'agents.opencode.model': 'Modell als provider/model angeben.',
      'agents.claude.extraArgs': 'Keine Anführungszeichen.',
      'docker.images.codex': 'Ungültiges Image.',
      'docker.ports': 'Host-Port 3000 ist doppelt belegt.',
    });
  });
});

describe('launch ports', () => {
  it('checks Docker ports and refuses ports for any other environment', () => {
    expect(launchPortsProblem({ env: 'docker', ports: [] })).toBe('');
    expect(
      launchPortsProblem({ env: 'docker', ports: ['127.0.0.1:3000:3000'] }),
    ).toBe('');
    expect(launchPortsProblem({ env: 'docker', ports: ['x'] })).toBe(
      'Port „x“ ist ungültig.',
    );
    expect(launchPortsProblem({ env: 'local', ports: [] })).toBe('');
    expect(
      launchPortsProblem({ env: 'local', ports: ['127.0.0.1:3000:3000'] }),
    ).toBe('Ports lassen sich nur für Docker-Sessions freigeben.');
    // Invalid ports come first, and blanks publish nothing, as natively.
    expect(launchPortsProblem({ env: 'local', ports: ['x'] })).toBe(
      'Port „x“ ist ungültig.',
    );
    expect(launchPortsProblem({ env: 'local', ports: [' '] })).toBe('');
  });
});
