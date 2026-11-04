/*
 * Shared helpers of the capture scripts. They drive the browser demo
 * (`pnpm dev`) on simulated data only: every capture asserts demo mode and
 * fails on any request that leaves the local dev server.
 */
import { chromium, expect } from '@playwright/test';
import { fileURLToPath } from 'node:url';

/** @typedef {import('@playwright/test').Page} Page */
/** @typedef {'dark' | 'light'} Theme */

export const root = fileURLToPath(new URL('../', import.meta.url));
export const baseUrl = process.env.REVIEW_BASE_URL ?? 'http://127.0.0.1:1420';
/** Both themes, each reached the way a user gets it: the OS preference. */
/** @type {readonly Theme[]} */
export const THEMES = ['dark', 'light'];

export function launch() {
  return chromium.launch({
    ...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE
      ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE }
      : {}),
    headless: true,
  });
}

/**
 * A fresh demo panel in its own context (fresh localStorage), with the OS
 * preferring `theme`. Asserts demo mode and the theme before returning.
 * @param {import('@playwright/test').Browser} browser
 * @param {{ theme: Theme; width?: number; height?: number }} options
 */
export async function openDemo(browser, { theme, width = 480, height = 760 }) {
  const context = await browser.newContext({
    viewport: { width, height },
    deviceScaleFactor: 2,
    locale: 'de-DE',
    colorScheme: theme,
    reducedMotion: 'reduce',
  });
  const page = await context.newPage();
  /** @type {string[]} */
  const externalRequests = [];
  page.on('request', (request) => {
    if (!['127.0.0.1', 'localhost'].includes(new URL(request.url()).hostname))
      externalRequests.push(request.url());
  });
  await page.goto(baseUrl);
  await expectDemo(page);
  expect(
    await page.evaluate(
      () => getComputedStyle(document.documentElement).colorScheme,
    ),
  ).toBe(theme);
  return { context, page, externalRequests };
}

/** The panel runs the browser demo, never the native shell. @param {Page} page */
export async function expectDemo(page) {
  await expect(page.getByRole('status', { name: 'Status' })).toContainText(
    'Demo',
  );
  await expect(
    page.getByRole('button', { name: 'Einstellungen', exact: true }),
  ).toBeEnabled();
}

/** @param {string[]} requests */
export function assertLocalOnly(requests) {
  if (requests.length)
    throw new Error(`Unexpected external network requests: ${requests.length}`);
}
