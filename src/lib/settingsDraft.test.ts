import { describe, expect, it } from 'vitest';
import { demoSettings } from './demoData';
import { envsProblem, lastEnv, shownEnv, toggleEnv } from './environments';
import { SettingsDraft } from './settingsDraft.svelte';
import {
  argsProblem,
  folderProblem,
  imageProblem,
  modelProblem,
} from './validation';

describe('validation', () => {
  it('accepts plain extra arguments and refuses quotes and control characters', () => {
    expect(argsProblem('')).toBe('');
    expect(argsProblem('--verbose --add-dir C:\\x')).toBe('');
    expect(argsProblem('--name "x"')).toBe('Keine Anführungszeichen.');
    expect(argsProblem("--name 'x'")).toBe('Keine Anführungszeichen.');
    expect(argsProblem('a\nb')).toBe('Keine Steuerzeichen.');
  });
  it('checks models, images and folders', () => {
    expect(modelProblem('opencode/big-pickle')).toBe('');
    expect(modelProblem('two words')).toBe('Ungültiges Modell.');
    expect(imageProblem('node:22-trixie')).toBe('');
    expect(imageProblem(' ')).toBe('Image fehlt.');
    expect(imageProblem('node 22')).toBe('Ungültiges Image.');
    expect(folderProblem('C:\\Users\\demo')).toBe('');
    expect(folderProblem('\\\\server\\share')).toBe('');
    expect(folderProblem('projekte')).toBe(
      'Ordner muss ein absoluter Windows-Pfad sein.',
    );
  });
});

describe('SettingsDraft', () => {
  it('stages edits: dirty until saved, never touching the baseline', () => {
    const saved = demoSettings();
    const draft = new SettingsDraft(saved);
    expect(draft.dirty).toBe(false);
    draft.value.agents.codex.permission = 'bypass';
    draft.value.docker.ghToken = true;
    draft.value.sharedWindow = true;
    expect(draft.dirty).toBe(true);
    expect(saved.agents.codex.permission).toBe('ask');
    expect(saved.sharedWindow).toBe(false);
    const current = draft.current();
    expect(current.agents.codex.permission).toBe('bypass');
    expect(current.sharedWindow).toBe(true);
    draft.commit(current);
    expect(draft.dirty).toBe(false);
  });

  it('marks invalid fields per tab group', () => {
    const draft = new SettingsDraft(demoSettings());
    draft.value.agents.claude.extraArgs = '--x "y"';
    expect(draft.invalid).toBe(true);
    expect(draft.invalidIn('agents')).toBe(true);
    expect(draft.invalidIn('docker')).toBe(false);
    expect(draft.problem('agents.claude.extraArgs')).toBe(
      'Keine Anführungszeichen.',
    );
    draft.value.agents.claude.extraArgs = '';
    draft.value.docker.images.opencode = '';
    expect(draft.invalidIn('agents')).toBe(false);
    expect(draft.invalidIn('docker')).toBe(true);
  });

  it('stages the shown environments; the default follows a hidden one', () => {
    const draft = new SettingsDraft(demoSettings());
    draft.value.defaultEnv = 'docker';
    toggleEnv(draft.value, 'docker', false);
    expect(draft.value.environments).toEqual(['local', 'wsl']);
    expect(draft.value.defaultEnv).toBe('local');
    // Lokal is the last one that starts sessions: WSL is not available yet.
    expect(lastEnv(draft.value.environments, 'local')).toBe(true);
    expect(lastEnv(draft.value.environments, 'wsl')).toBe(false);
    toggleEnv(draft.value, 'wsl', false);
    toggleEnv(draft.value, 'docker', true);
    expect(draft.current().environments).toEqual(['local', 'docker']);
    expect(lastEnv(draft.value.environments, 'local')).toBe(false);
    expect(draft.invalid).toBe(false);
    expect(draft.dirty).toBe(true);
  });

  it('hides Docker with the list; unticking resets only its invalid fields', () => {
    const saved = demoSettings();
    const draft = new SettingsDraft(saved);
    expect(draft.dockerInView).toBe(true);
    draft.value.docker.images.codex = 'node:24-trixie';
    draft.value.docker.images.claude = '';
    draft.value.docker.ports = ['70000'];
    expect(draft.invalidIn('docker')).toBe(true);
    draft.toggleEnv('docker', false);
    expect(draft.dockerInView).toBe(false);
    // Nothing hidden blocks „Speichern“; the valid edit stays staged.
    expect(draft.invalid).toBe(false);
    expect(draft.current().docker).toMatchObject({
      images: { codex: 'node:24-trixie', claude: saved.docker.images.claude },
      ports: saved.docker.ports,
    });
    draft.toggleEnv('docker', true);
    expect(draft.dockerInView).toBe(true);
    expect(draft.current().docker.images.codex).toBe('node:24-trixie');
    // Hiding another environment touches no Docker field.
    draft.value.docker.images.opencode = '';
    draft.toggleEnv('wsl', false);
    expect(draft.problem('docker.images.opencode')).toBe('Image fehlt.');
  });

  it('mirrors the native environment rules', () => {
    const settings = demoSettings();
    expect(envsProblem({ ...settings, environments: [] })).toBe(
      'Mindestens eine Umgebung muss aktiv sein.',
    );
    expect(envsProblem({ ...settings, environments: ['wsl'] })).toBe(
      'WSL ist noch nicht verfügbar.',
    );
    expect(envsProblem({ ...settings, environments: ['docker'] })).toBe(
      'Die Standard-Umgebung ist deaktiviert.',
    );
    const draft = new SettingsDraft(settings);
    draft.value.environments = [];
    expect(draft.invalidIn('environments')).toBe(true);
    expect(draft.invalidIn('docker')).toBe(false);
    expect(shownEnv('docker', ['local'])).toBe('local');
    // A list without a usable environment (never saved) shows all of them.
    expect(shownEnv('docker', [])).toBe('docker');
  });

  it('stages the clone root: empty is the default, else an absolute folder', () => {
    const saved = demoSettings();
    const draft = new SettingsDraft(saved);
    expect(saved.github.cloneRoot).toBe('');
    draft.value.github.cloneRoot = 'repos';
    expect(draft.dirty).toBe(true);
    expect(draft.invalidIn('github')).toBe(true);
    expect(draft.problem('github.cloneRoot')).toBe(
      'Ordner muss ein absoluter Windows-Pfad sein.',
    );
    draft.value.github.cloneRoot = 'D:\\Code';
    expect(draft.invalid).toBe(false);
    expect(draft.current().github.cloneRoot).toBe('D:\\Code');
    expect(saved.github.cloneRoot).toBe('');
    draft.value.github.cloneRoot = '  ';
    expect(draft.invalidIn('github')).toBe(false);
  });
});
