import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';

export default defineConfig({
  plugins: [sveltekit(), wasm()],
  // Mobile dev path; Tauri overrides `devUrl` via tauri.conf.json on android.
  server: { port: 1421, strictPort: true, host: '0.0.0.0' },
  clearScreen: false,
});
