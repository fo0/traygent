import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { SIGNAL_BARS, effortLevel, modelLetter } from './optionMarks';
import type { Snapshot } from './types';

/** The Rust catalog, through the golden fixture its test writes. */
const { catalog } = JSON.parse(
  readFileSync(join(process.cwd(), 'fixtures/snapshot.default.json'), 'utf8'),
) as Snapshot;
const ids = (agent: string, list: 'models' | 'efforts') =>
  catalog
    .find((entry) => entry.id === agent)!
    [list].map((choice) => choice.id)
    .filter((id) => id !== 'default');

describe('effortLevel', () => {
  it('fills one more bar per catalog level, all bars at the top', () => {
    for (const entry of catalog) {
      const levels = entry.efforts.map((choice) => effortLevel(choice.id));
      expect(levels).toEqual([0, 1, 2, 3, 4, 5]);
    }
    expect(effortLevel('max')).toBe(SIGNAL_BARS);
  });
  it('fills none for „Standard“ and an id it does not know', () => {
    expect(effortLevel('default')).toBe(0);
    expect(effortLevel('turbo')).toBe(0);
    expect(effortLevel('toString')).toBe(0);
  });
});

describe('modelLetter', () => {
  it('names the Claude models by their family', () => {
    expect(ids('claude', 'models').map(modelLetter)).toEqual([
      'F', // Fable
      'O', // Opus
      'S', // Sonnet
      'H', // Haiku
      'P', // Opus Plan: a mode of its own
    ]);
  });
  it('names the Codex models by the part after their shared prefix', () => {
    expect(
      Object.fromEntries(
        ids('codex', 'models').map((id) => [id, modelLetter(id)]),
      ),
    ).toEqual({
      'gpt-6.1-sol': 'S',
      'gpt-6-astra': 'A',
      'gpt-6-sol': 'S',
      'gpt-6-luna': 'L',
      'gpt-5.6-sol': 'S',
      'gpt-6.1-terra': 'T',
      'gpt-5.6-terra': 'T',
      'gpt-5.6-luna': 'L',
      'gpt-5.5': 'G',
    });
  });
  it('finds a family word inside a free model string', () => {
    expect(modelLetter('claude-opus-5-5')).toBe('O');
    expect(modelLetter('anthropic/claude-sonnet-4')).toBe('S');
    expect(modelLetter('sonnet-latest')).toBe('S');
    expect(modelLetter('openai/gpt-6.1-terra')).toBe('T');
    expect(modelLetter('Haiku')).toBe('H');
  });
  it('names any other model by the first word of its own name', () => {
    expect(modelLetter('google/gemini-2.5-flash')).toBe('G');
    expect(modelLetter('opencode/big-pickle')).toBe('B');
    expect(modelLetter('mistral-large-2')).toBe('M');
    expect(modelLetter('4o')).toBe('O');
  });
  it('falls back to the provider, then to the first character', () => {
    expect(modelLetter('openrouter/4.1')).toBe('O');
    expect(modelLetter('123')).toBe('1');
    expect(modelLetter('  ')).toBe('?');
  });
});
