import { sveltekit } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [sveltekit()],
  // Mobile dev path; Tauri overrides `devUrl` via tauri.conf.json on android.
  server: { port: 1421, strictPort: true, host: '0.0.0.0' },
  clearScreen: false,
});
