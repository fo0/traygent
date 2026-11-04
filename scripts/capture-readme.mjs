/*
 * README screenshots: the real panel of the browser demo (`pnpm dev`, simulated
 * data: composer, recent sessions, Docker state) in the dark and the light theme,
 * each placed above the notification area of a generic, illustrative desktop
 * taskbar. The frame is drawn here from plain shapes (no OS logo or artwork);
 * the panel is embedded pixel for pixel at the capture's scale. Output:
 * docs/images/traygent-<theme>.png, committed and shown in README.md.
 * Needs `pnpm dev` running; REVIEW_BASE_URL targets another port.
 */
import { expect } from '@playwright/test';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import {
  THEMES,
  assertLocalOnly,
  launch,
  openDemo,
  root,
} from './capture-support.mjs';

/** The panel's tray size (tauri.conf.json → app.windows[0]). */
const PANEL = { width: 480, height: 760 };
const SCALE = 2;
/** Desktop frame around the panel, in CSS pixels. */
const FRAME = { width: 680, wallpaper: 808, taskbar: 48, gap: 12 };
const output = path.join(root, 'docs/images');

const icon = await readFile(path.join(root, 'src-tauri/icons/source.svg'));
const iconUri = `data:image/svg+xml;base64,${icon.toString('base64')}`;

/** Per-theme colors of the generic desktop; not part of the app. */
const LOOK = {
  dark: {
    desktop: '#161a23',
    glowA: '#2b3a55',
    glowB: '#1f4a45',
    taskbar: 'rgba(28, 30, 36, 0.92)',
    edge: 'rgba(255, 255, 255, 0.08)',
    glyph: '#e6e7ea',
    tile: 'rgba(255, 255, 255, 0.14)',
    highlight: 'rgba(255, 255, 255, 0.12)',
    accent: '#b9a5ff',
    shadow: '0 18px 48px rgba(0, 0, 0, 0.55), 0 0 0 1px rgba(255,255,255,0.06)',
  },
  light: {
    desktop: '#eef0f4',
    glowA: '#d9e4f7',
    glowB: '#d3eee3',
    taskbar: 'rgba(243, 244, 247, 0.94)',
    edge: 'rgba(0, 0, 0, 0.08)',
    glyph: '#24262b',
    tile: 'rgba(0, 0, 0, 0.12)',
    highlight: 'rgba(0, 0, 0, 0.07)',
    accent: '#5b3fc4',
    shadow: '0 18px 48px rgba(30, 40, 60, 0.28), 0 0 0 1px rgba(0,0,0,0.07)',
  },
};

/** Generic tray glyphs from basic strokes (overflow, network, volume, battery). */
const OVERFLOW =
  '<path d="M6 13l4-4 4 4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>';
const STATUS = [
  '<path d="M3 8.5a10 10 0 0 1 14 0M5.5 11a6.5 6.5 0 0 1 9 0M8 13.5a3 3 0 0 1 4 0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><circle cx="10" cy="16" r="1.2" fill="currentColor"/>',
  '<path d="M3 8h3l4-3.5v11L6 12H3z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><path d="M13 7.5a3.5 3.5 0 0 1 0 5M15 5.5a6.5 6.5 0 0 1 0 9" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>',
  '<rect x="2.5" y="6.5" width="13" height="7" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.4"/><rect x="4.3" y="8.3" width="7.5" height="3.4" rx="0.6" fill="currentColor"/><rect x="16.3" y="8.6" width="1.6" height="2.8" rx="0.6" fill="currentColor"/>',
];
const glyph = (/** @type {string} */ body) =>
  `<span class="glyph"><svg viewBox="0 0 20 20" width="18" height="18" aria-hidden="true">${body}</svg></span>`;

/**
 * A soft radial glow from flat, translucent concentric circles. Unlike a CSS
 * gradient or blur it is not dithered, so the PNG stays small, and the rings
 * blend into whatever lies beneath.
 * @param {number} cx
 * @param {number} cy
 * @param {number} radius
 * @param {string} color
 */
function glow(cx, cy, radius, color, steps = 60) {
  return Array.from(
    { length: steps },
    (_, step) =>
      `<circle cx="${cx}" cy="${cy}" r="${(radius * (1 - step / steps)).toFixed(1)}" fill="${color}" fill-opacity="0.04"/>`,
  ).join('');
}

/**
 * The desktop frame with the panel capture above the tray icon.
 * @param {import('./capture-support.mjs').Theme} theme
 * @param {Buffer} panel
 * @param {{ time: string; date: string }} clock
 */
function frameHtml(theme, panel, clock) {
  const look = LOOK[theme];
  const panelUri = `data:image/png;base64,${panel.toString('base64')}`;
  return `<!doctype html><html><head><meta charset="utf-8"><style>
  * { box-sizing: border-box; margin: 0; }
  html, body { width: ${FRAME.width}px; height: ${FRAME.wallpaper + FRAME.taskbar}px; }
  body { position: relative; overflow: hidden; background: ${look.desktop};
    font: 12px/1.25 "Segoe UI", system-ui, sans-serif; color: ${look.glyph}; }
  .wallpaper { position: absolute; inset: 0; }
  .panel { position: absolute; right: ${FRAME.gap}px; bottom: ${FRAME.taskbar + FRAME.gap}px;
    width: ${PANEL.width}px; height: ${PANEL.height}px; border-radius: 10px;
    overflow: hidden; box-shadow: ${look.shadow}; }
  .panel img { display: block; width: ${PANEL.width}px; height: ${PANEL.height}px; }
  .taskbar { position: absolute; left: 0; right: 0; bottom: 0; height: ${FRAME.taskbar}px;
    display: flex; align-items: center; padding: 0 8px 0 12px; gap: 6px;
    background: ${look.taskbar}; border-top: 1px solid ${look.edge}; }
  .apps { display: flex; gap: 8px; margin-right: auto; }
  .apps span { width: 26px; height: 26px; border-radius: 6px; background: ${look.tile}; }
  .glyph, .app-icon { display: grid; place-items: center; width: 30px; height: 36px; border-radius: 6px; }
  .app-icon { position: relative; background: ${look.highlight}; }
  .app-icon img { width: 20px; height: 20px; }
  .app-icon::after { content: ""; position: absolute; left: 10px; right: 10px; bottom: 3px;
    height: 3px; border-radius: 2px; background: ${look.accent}; }
  .clock { text-align: right; padding: 0 8px 0 6px; font-size: 12px; }
  </style></head><body>
  <svg class="wallpaper" width="${FRAME.width}" height="${FRAME.wallpaper}" aria-hidden="true">
    ${glow(120, 140, 560, look.glowA)}
    ${glow(620, 760, 520, look.glowB)}
  </svg>
  <div class="panel"><img src="${panelUri}" alt=""></div>
  <div class="taskbar">
    <div class="apps"><span></span><span></span><span></span><span></span></div>
    ${glyph(OVERFLOW)}
    <span class="app-icon"><img src="${iconUri}" alt=""></span>
    ${STATUS.map(glyph).join('')}
    <div class="clock">${clock.time}<br>${clock.date}</div>
  </div>
  </body></html>`;
}

await mkdir(output, { recursive: true });
const browser = await launch();
/** @type {string[]} */
const externalRequests = [];
try {
  for (const theme of THEMES) {
    const demo = await openDemo(browser, { theme, ...PANEL });
    const { page } = demo;
    // The dashboard as it opens: the „Neue Session“ composer from the saved
    // defaults, the demo's recent sessions and the footer's Docker state and
    // „Stand“ of the finished startup detection. No notice, no spinner.
    await expect(
      page.getByRole('heading', { name: 'Zuletzt', level: 2 }),
    ).toBeVisible();
    await expect(page.locator('form.composer')).toBeVisible();
    await expect(page.locator('.recent-row')).toHaveCount(6);
    await expect(page.locator('.docker-state')).toHaveText(
      'Docker 27.5 · bereit',
    );
    await expect(page.getByText(/^Stand \d\d:\d\d$/)).toBeVisible();
    await expect(page.locator('.notice-banner, .spinning')).toHaveCount(0);
    await page.mouse.move(0, 0);
    const panel = await page.screenshot({ animations: 'disabled' });
    // The taskbar clock shows the panel's „Stand“ time, so both agree.
    const stand = await page.getByText(/^Stand \d\d:\d\d$/).textContent();
    const clock = {
      time: (stand ?? '').replace('Stand ', ''),
      date: new Date().toLocaleDateString('de-DE', {
        day: '2-digit',
        month: '2-digit',
        year: 'numeric',
      }),
    };
    externalRequests.push(...demo.externalRequests);
    await demo.context.close();

    const context = await browser.newContext({
      viewport: {
        width: FRAME.width,
        height: FRAME.wallpaper + FRAME.taskbar,
      },
      deviceScaleFactor: SCALE,
      colorScheme: theme,
    });
    const frame = await context.newPage();
    frame.on('request', (request) => {
      if (!request.url().startsWith('data:'))
        externalRequests.push(request.url());
    });
    await frame.setContent(frameHtml(theme, panel, clock));
    // The embedded capture must keep its exact pixel size: one image pixel
    // per device pixel, no rescaling.
    const natural = await frame
      .locator('.panel img')
      .evaluate((/** @type {HTMLImageElement} */ img) => [
        img.naturalWidth,
        img.naturalHeight,
      ]);
    expect(natural).toEqual([PANEL.width * SCALE, PANEL.height * SCALE]);
    const file = path.join(output, `traygent-${theme}.png`);
    await writeFile(file, await frame.screenshot({ animations: 'disabled' }));
    await context.close();
    console.log(`Captured ${path.relative(root, file)}`);
  }
  assertLocalOnly(externalRequests);
} finally {
  await browser.close();
}
