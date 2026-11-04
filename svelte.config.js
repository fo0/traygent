/**
 * Read by vite-plugin-svelte, svelte-check and eslint-plugin-svelte.
 * @type {import('@sveltejs/vite-plugin-svelte').SvelteConfig}
 */
export default {
  // Svelte 5 runes mode for every component: legacy `export let`, `$:` or
  // `on:` syntax fails to compile instead of switching that component back.
  compilerOptions: { runes: true },
};
