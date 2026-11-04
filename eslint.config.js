// @ts-check
import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import svelte from 'eslint-plugin-svelte';
import { defineConfig, globalIgnores } from 'eslint/config';
import globals from 'globals';
import ts from 'typescript-eslint';
import svelteConfig from './svelte.config.js';

// Code-quality rules only: formatting stays with Prettier (`pnpm lint` runs both).
export default defineConfig(
  globalIgnores([
    'dist/',
    'src-tauri/target/',
    'src-tauri/gen/',
    'playwright-report/',
    'test-results/',
    'release-work/',
    'docs/review/',
    '.claude/worktrees/',
  ]),
  js.configs.recommended,
  ts.configs.recommended,
  svelte.configs.recommended,
  prettier,
  svelte.configs.prettier,
  // Panel code runs in the WebView; everything else runs in Node.
  { files: ['src/**'], languageOptions: { globals: globals.browser } },
  {
    files: ['src/**/*.test.ts', 'tests/**', 'scripts/**', '*.config.{js,ts}'],
    languageOptions: { globals: globals.node },
  },
  {
    // Screenshot scripts measure the page inside page.evaluate callbacks.
    files: ['scripts/capture-*.mjs'],
    languageOptions: { globals: globals.browser },
  },
  {
    files: ['**/*.svelte', '**/*.svelte.ts'],
    languageOptions: {
      parserOptions: {
        parser: ts.parser,
        extraFileExtensions: ['.svelte'],
        svelteConfig,
      },
    },
  },
  {
    // The stores keep Map/Set in `$state.raw` and replace them copy-on-write
    // (`new Map(old).set(…)`), so assignment is the update; private
    // bookkeeping stays deliberately non-reactive. SvelteMap/SvelteSet would
    // make every mutation reactive and change update timing.
    files: ['src/**/*.svelte.ts'],
    rules: { 'svelte/prefer-svelte-reactivity': 'off' },
  },
);
