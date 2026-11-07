import { describe, expect, it } from 'vitest';
import { matchChoices, toItems } from './choices';

const MODELS = [
  { id: 'opus', label: 'opus', meta: 'Opus 5.5' },
  { id: 'opusplan', label: 'opusplan', meta: 'Opus Plan' },
  { id: 'sonnet', label: 'sonnet', meta: 'Sonnet 5.5' },
  { id: 'haiku', label: 'haiku' },
];

describe('matchChoices', () => {
  it('keeps every item for an empty or blank query', () => {
    expect(matchChoices(MODELS, '')).toEqual(MODELS);
    expect(matchChoices(MODELS, '   ')).toEqual(MODELS);
  });

  it('matches label and meta, ignoring case, in catalog order', () => {
    expect(matchChoices(MODELS, 'OPUS').map((m) => m.id)).toEqual([
      'opus',
      'opusplan',
    ]);
    expect(matchChoices(MODELS, '5.5').map((m) => m.id)).toEqual([
      'opus',
      'sonnet',
    ]);
  });

  it('needs every word', () => {
    expect(matchChoices(MODELS, 'opus plan').map((m) => m.id)).toEqual([
      'opusplan',
    ]);
    expect(matchChoices(MODELS, 'haiku 5.5')).toEqual([]);
  });
});

describe('toItems', () => {
  const EFFORTS = [
    { id: 'default', label: 'Standard', hint: 'Ohne --effort' },
    { id: 'high', label: 'Hoch' },
  ];

  it('maps the hint to the tooltip', () => {
    expect(toItems(EFFORTS)).toEqual([
      { id: 'default', label: 'Standard', title: 'Ohne --effort' },
      { id: 'high', label: 'Hoch', title: undefined },
    ]);
  });

  it('keeps an unknown stored value selectable, first', () => {
    expect(toItems(EFFORTS, 'ultra').map((item) => item.id)).toEqual([
      'ultra',
      'default',
      'high',
    ]);
    expect(toItems(EFFORTS, 'high')).toHaveLength(2);
  });
});
