import { readdirSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

/**
 * Windows resolves imports case-insensitively: `./composer.svelte` once
 * resolved to `Composer.svelte` instead of `composer.svelte.ts` in the
 * Windows CI build. No two modules may share a name once case and the `.ts`
 * suffix are ignored.
 */
describe('module file names', () => {
  it('stay unique on a case-insensitive file system', () => {
    for (const dir of ['src/lib', 'src']) {
      const names = readdirSync(dir).map((name) =>
        name.replace(/\.ts$/, '').toLowerCase(),
      );
      const duplicates = names.filter(
        (name, index) => names.indexOf(name) !== index,
      );
      expect(duplicates, dir).toEqual([]);
    }
  });
});
