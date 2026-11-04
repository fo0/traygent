import { test, type Page } from './test';

/**
 * Fonts the layout specs pin, whatever the runner's default is: Liberation
 * Sans has Segoe UI's width class and reaches CI's Ubuntu runner through
 * `playwright install --with-deps`; DejaVu Sans, a wider fallback, ships with
 * Ubuntu.
 */
export const LAYOUT_FONTS = ['Liberation Sans', 'DejaVu Sans'] as const;
export type LayoutFont = (typeof LAYOUT_FONTS)[number];

/**
 * Renders the page in `font`. The fallback is monospace, so a missing font
 * never passes as the default one: the test is skipped with the reason.
 */
export async function pinFont(page: Page, font: string) {
  const installed = await page.evaluate((family) => {
    const context = document.createElement('canvas').getContext('2d')!;
    const width = (stack: string) => {
      context.font = `16px ${stack}`;
      return context.measureText('Claude Code · OpenCode · Codex 0123').width;
    };
    return width(`'${family}', monospace`) !== width('monospace');
  }, font);
  test.skip(!installed, `${font} is not installed on this machine`);
  await page.addStyleTag({
    content: `:root { font-family: '${font}', monospace !important; }`,
  });
}
