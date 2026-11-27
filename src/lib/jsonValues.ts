// Structural helpers for the plain JSON values the settings and the composer
// stage (no Map, Set or class instances).

/** Whether `a` and `b` serialize alike: same keys in the same order, same values. */
export const same = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);
const plain = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === 'object' && !Array.isArray(value);

/**
 * `value` moved from the baseline `from` onto `to`: whatever still equals
 * `from` takes `to`'s value, a staged edit stays. Objects merge key by key,
 * lists and scalars are one field each.
 */
export function rebase<T>(from: T, to: T, value: T): T {
  if (same(value, from)) return structuredClone(to);
  if (!plain(from) || !plain(to) || !plain(value)) return value;
  const merged: Record<string, unknown> = {};
  for (const key of new Set([...Object.keys(to), ...Object.keys(value)])) {
    const next = rebase(from[key], to[key], value[key]);
    if (next !== undefined) merged[key] = next;
  }
  return merged as T;
}
