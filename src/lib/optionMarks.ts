/*
 * Session options as symbols (owner request, #17): the thinking level as
 * signal bars, the model as a badge with one letter. The labels stay in the
 * tooltips and accessible names; these only pick the symbol.
 */

/** Bars of the thinking-level glyph: one per catalog level above „Standard“. */
export const SIGNAL_BARS = 5;

/** Filled bars per effort id of the catalog (`catalog.rs → efforts`). */
const EFFORT_LEVELS: Readonly<Record<string, number>> = {
  low: 1,
  medium: 2,
  high: 3,
  xhigh: 4,
  max: 5,
};

/** How many bars an effort fills; „Standard“ and unknown ids fill none. */
export function effortLevel(effort: string): number {
  return Object.hasOwn(EFFORT_LEVELS, effort) ? EFFORT_LEVELS[effort]! : 0;
}

/**
 * Model families by the word that names them, and their letter. Codex models
 * share the `gpt-` prefix, so their distinctive last word counts; a mode
 * such as Opus Plan gets a letter of its own instead of its base model's.
 */
const FAMILIES: Readonly<Record<string, string>> = {
  opus: 'O',
  sonnet: 'S',
  haiku: 'H',
  fable: 'F',
  opusplan: 'P',
  sol: 'S',
  astra: 'A',
  terra: 'T',
  luna: 'L',
};

/**
 * The badge letter of a model id: a known family word anywhere in it (the
 * last one wins), else the first letter of the first alphabetic word after
 * the last `/` — the model's own name, not its provider's
 * (`google/gemini-2.5-flash` → G, `opencode/big-pickle` → B, `gpt-5.5` → G)
 * — else of the id's first word, else its first character. A free model
 * string always gets one.
 */
export function modelLetter(model: string): string {
  const words = model.toLowerCase().match(/[a-z]+/g) ?? [];
  const family = [...words]
    .reverse()
    .find((word) => Object.hasOwn(FAMILIES, word));
  if (family) return FAMILIES[family]!;
  const name = model.slice(model.lastIndexOf('/') + 1);
  const first = /[a-z]+/i.exec(name)?.[0] ?? words[0];
  if (first) return first[0]!.toUpperCase();
  return model.trim().charAt(0).toUpperCase() || '?';
}
