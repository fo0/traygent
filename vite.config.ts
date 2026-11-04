import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  // Compiler options (runes mode) come from svelte.config.js.
  plugins: [svelte()],
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    // Cargo writes to src-tauri/target during `tauri dev`; never reload on it.
    watch: { ignored: ['**/src-tauri/**'] },
  },
  // The panel only ships inside WebView2, an evergreen Chromium on Windows.
  build: { target: 'chrome120' },
  test: { include: ['src/**/*.test.ts'], environment: 'node' },
});
