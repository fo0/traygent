import { describe, expect, it } from 'vitest';
import { demoSettings } from './demoData';
import { DEBUG_TOOLTIP, LOG_LEVELS, visibleSignIn } from './logLevel';
import { SettingsDraft } from './settingsDraft.svelte';

describe('„Protokoll“ (#109)', () => {
  it('offers Info before Debug and starts at Info', () => {
    expect(LOG_LEVELS.map((level) => [level.value, level.label])).toEqual([
      ['info', 'Info'],
      ['debug', 'Debug'],
    ]);
    expect(demoSettings().logLevel).toBe('info');
    // The tooltip says what „Debug“ changes besides the log.
    expect(DEBUG_TOOLTIP).toContain('sichtbaren Terminal');
  });

  it('opens a sign-in visibly only at the saved level „Debug“', () => {
    const settings = demoSettings();
    expect(visibleSignIn(settings)).toBe(false);
    expect(visibleSignIn({ ...settings, logLevel: 'debug' })).toBe(true);
    // No snapshot yet: hidden, the default.
    expect(visibleSignIn(undefined)).toBe(false);
  });

  it('stages the level like every other setting', () => {
    const draft = new SettingsDraft(demoSettings());
    draft.value.logLevel = 'debug';
    expect(draft.dirty).toBe(true);
    expect(draft.saved.logLevel).toBe('info');
    expect(draft.current().logLevel).toBe('debug');
    draft.value.logLevel = 'info';
    expect(draft.dirty).toBe(false);
    draft.value.logLevel = 'debug';
    draft.commit(draft.current());
    expect(draft.dirty).toBe(false);
    expect(draft.saved.logLevel).toBe('debug');
  });
});
