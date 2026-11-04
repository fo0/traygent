import { defineConfig } from '@playwright/test';

// A distinct port lets several local checkouts run the suite side by side.
const port = Number(process.env.E2E_PORT ?? 1420);
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  // CI keeps annotations plus an HTML report (playwright-report/) that links
  // the retained traces in test-results/ for upload as workflow artifacts.
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL,
    viewport: { width: 500, height: 840 },
    // The suite runs in the dark theme, the default design; tests/theme.spec.ts
    // covers the light one and the override.
    colorScheme: 'dark',
    launchOptions: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE
      ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE }
      : {},
    trace: 'retain-on-failure',
  },
  webServer: {
    command: `node node_modules/vite/bin/vite.js --host 127.0.0.1 --port ${port} --strictPort`,
    url: baseURL,
    reuseExistingServer: !process.env.CI,
  },
});
