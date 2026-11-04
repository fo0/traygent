import {
  test as base,
  expect,
  type ConsoleMessage,
  type Page,
} from '@playwright/test';

/**
 * Shared Playwright entry point: every spec imports `test` and `expect` from
 * here. An automatic fixture fails the test on any uncaught page error, any
 * console error and any Svelte dev warning, so a crash that the UI happens to
 * survive (e.g. a Svelte `state_unsafe_mutation` thrown from an event
 * handler) never passes silently.
 */

/** The dev server serves no favicon; Chromium reports that 404 as an error. */
function expected(message: ConsoleMessage) {
  return /\/favicon\.ico$/.test(message.location().url);
}

/**
 * Collects page errors, console errors and Svelte warnings of `page`. Specs that
 * open their own context page call this and assert the list is empty at the end.
 */
export function guardConsole(page: Page): string[] {
  const problems: string[] = [];
  page.on('pageerror', (error) =>
    problems.push(`pageerror: ${error.stack ?? error.message}`),
  );
  page.on('console', (message) => {
    if (message.type() === 'error' && !expected(message))
      problems.push(`console.error: ${message.text()}`);
    // Svelte's dev warnings name real defects, e.g. a non-reactive binding.
    if (message.type() === 'warning' && message.text().includes('[svelte]'))
      problems.push(`svelte warning: ${message.text()}`);
  });
  return problems;
}

export const test = base.extend<{ cleanConsole: void }>({
  cleanConsole: [
    async ({ page }, use) => {
      const problems = guardConsole(page);
      await use();
      expect(problems, 'uncaught page errors or console errors').toEqual([]);
    },
    { auto: true },
  ],
});

export { expect };
export type { Locator, Page } from '@playwright/test';
