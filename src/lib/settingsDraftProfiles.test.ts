import { describe, expect, it, vi } from 'vitest';
import { demoSettings } from './demoData';
import { renameProblem } from './profiles';
import { SettingsDraft } from './settingsDraft.svelte';
import type { AppSettings } from './types';

/*
 * Profiles in the settings draft (#37): an add saves at once on top of the
 * baseline and keeps the staged edits; removals and renames stay staged.
 */
describe('SettingsDraft profiles', () => {
  it('adds a profile at once: the baseline plus it, other edits stay staged', async () => {
    const saved = demoSettings();
    const draft = new SettingsDraft(saved);
    draft.value.agents.codex.permission = 'bypass';
    draft.value.agentProfile.claude = 'team';
    const sent: AppSettings[] = [];
    let busy = false;
    const save = vi.fn(async (settings: AppSettings) => {
      busy = draft.adding && draft.busy;
      sent.push(settings);
      return structuredClone(settings);
    });
    expect(await draft.addProfile('claude', '  Kunde A ', save)).toBe('');
    expect(busy).toBe(true);
    expect(draft.adding).toBe(false);
    // Only the new profile was sent, never the staged edits.
    expect(sent[0]?.profiles.claude).toEqual([
      { id: 'team', label: 'Team' },
      { id: 'kunde-a', label: 'Kunde A' },
    ]);
    expect(sent[0]?.agents.codex.permission).toBe('ask');
    expect(sent[0]?.agentProfile.claude).toBe('default');
    // The answer is the baseline; the staged edits stay staged on top of it.
    expect(draft.saved.profiles.claude).toHaveLength(2);
    expect(draft.value.profiles.claude).toEqual(draft.saved.profiles.claude);
    expect(draft.current().agents.codex.permission).toBe('bypass');
    expect(draft.current().agentProfile.claude).toBe('team');
    draft.value.agents.codex.permission = 'ask';
    draft.value.agentProfile.claude = 'default';
    // The add alone leaves nothing to „Speichern“.
    expect(draft.dirty).toBe(false);
    expect(saved.profiles.claude).toHaveLength(1);
  });

  it('refuses a name before sending it and keeps the draft on a native refusal', async () => {
    const draft = new SettingsDraft(demoSettings());
    const save = vi.fn(async (): Promise<AppSettings> => {
      throw new Error('Höchstens 8 Profile für Claude Code möglich.');
    });
    expect(await draft.addProfile('claude', 'team', save)).toBe(
      'Profil gibt es schon.',
    );
    expect(await draft.addProfile('claude', 'Standard', save)).toBe(
      '„Standard“ ist reserviert.',
    );
    expect(save).not.toHaveBeenCalled();
    draft.value.sharedWindow = true;
    expect(await draft.addProfile('claude', 'Kunde', save)).toBe(
      'Höchstens 8 Profile für Claude Code möglich.',
    );
    expect(draft.adding).toBe(false);
    expect(draft.value.profiles.claude).toEqual([
      { id: 'team', label: 'Team' },
    ]);
    expect(draft.current().sharedWindow).toBe(true);
    // While „Speichern“ runs, an add waits and sends nothing.
    draft.saving = true;
    expect(await draft.addProfile('claude', 'Neu', save)).toBeNull();
    expect(save).toHaveBeenCalledOnce();
  });

  it('brings a profile removed in the draft back once, without a save', async () => {
    const draft = new SettingsDraft(demoSettings());
    const save = vi.fn(async (settings: AppSettings) => settings);
    expect(await draft.addProfile('claude', 'Kunde', save)).toBe('');
    draft.removeProfile('claude', 'team');
    expect(draft.dirty).toBe(true);
    expect(await draft.addProfile('claude', 'Team', save)).toBe('');
    expect(save).toHaveBeenCalledOnce();
    // Back at its saved place, not doubled.
    expect(draft.value.profiles.claude).toEqual([
      { id: 'team', label: 'Team' },
      { id: 'kunde', label: 'Kunde' },
    ]);
    expect(draft.dirty).toBe(false);
    expect(await draft.addProfile('claude', 'team', save)).toBe(
      'Profil gibt es schon.',
    );
    expect(save).toHaveBeenCalledOnce();
  });

  it('waits for „Speichern“ when only a staged rename frees the name', async () => {
    const settings = demoSettings();
    settings.profiles.claude = [{ id: 'x1', label: 'Kunde' }];
    const draft = new SettingsDraft(settings);
    const save = vi.fn(async (value: AppSettings) => value);
    draft.value.profiles.claude[0]!.label = 'Alt';
    expect(await draft.addProfile('claude', 'Kunde', save)).toBe(
      'Profil gibt es schon. Erst speichern.',
    );
    expect(save).not.toHaveBeenCalled();
  });

  it('adds next to a staged removal: it stays staged, the new one is saved', async () => {
    const draft = new SettingsDraft(demoSettings());
    const save = vi.fn(async (settings: AppSettings) => settings);
    draft.value.agentProfile.claude = 'team';
    draft.removeProfile('claude', 'team');
    expect(await draft.addProfile('claude', 'Kunde', save)).toBe('');
    expect(save.mock.calls[0]?.[0].profiles.claude).toEqual([
      { id: 'team', label: 'Team' },
      { id: 'kunde', label: 'Kunde' },
    ]);
    expect(draft.value.profiles.claude).toEqual([
      { id: 'kunde', label: 'Kunde' },
    ]);
    expect(draft.value.agentProfile.claude).toBe('default');
    expect(draft.dirty).toBe(true);
    draft.removeProfile('claude', 'kunde');
    expect(draft.value.profiles.claude).toEqual([]);
    expect(draft.invalid).toBe(false);
  });

  it('moves the staged edits onto a new baseline field by field', () => {
    const draft = new SettingsDraft(demoSettings());
    draft.value.agents.codex.permission = 'bypass';
    const answer = structuredClone(draft.saved);
    answer.agents.codex.permission = 'auto';
    answer.agents.claude.extraArgs = '--verbose';
    answer.profiles.claude.push({ id: 'neu', label: 'Neu' });
    draft.adopt(answer, 'claude', 'neu');
    // An unedited field follows the answer, an edited one stays staged.
    expect(draft.value.agents.claude.extraArgs).toBe('--verbose');
    expect(draft.value.agents.codex.permission).toBe('bypass');
    expect(draft.value.profiles.claude.map((p) => p.id)).toEqual([
      'team',
      'neu',
    ]);
    draft.value.agents.codex.permission = 'auto';
    expect(draft.dirty).toBe(false);
  });

  it('stages a rename: id and folder stay, the label follows the rules', () => {
    const draft = new SettingsDraft(demoSettings());
    const team = draft.value.profiles.claude[0]!;
    team.label = 'Kunde B';
    expect(draft.dirty).toBe(true);
    expect(draft.invalid).toBe(false);
    expect(draft.current().profiles.claude).toEqual([
      { id: 'team', label: 'Kunde B' },
    ]);
    team.label = '';
    expect(renameProblem(draft.value.profiles.claude, 0)).toBe('Name fehlt.');
    expect(draft.problem('agents.claude.profiles')).toBe(': Name fehlt.');
    team.label = 'standard';
    expect(renameProblem(draft.value.profiles.claude, 0)).toBe(
      '„standard“ ist reserviert.',
    );
    expect(draft.invalidIn('agents')).toBe(true);
    team.label = 'Team';
    expect(draft.dirty).toBe(false);
  });
});
