import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

function emptyWasmPlugin() {
  return {
    name: 'empty-wasm',
    resolveId(id) {
      if (id === 'virtual:empty-office-wasm' || id === 'virtual:empty-archive-wasm' || id === 'virtual:empty-pptx-wasm') {
        return id;
      }
    },
    load(id) {
      if (id === 'virtual:empty-office-wasm') {
        return `export default async function() { throw new Error('Office WASM not available on mobile'); }`;
      }
      if (id === 'virtual:empty-archive-wasm') {
        return `export default async function() { throw new Error('Archive WASM not available on mobile'); }`;
      }
      if (id === 'virtual:empty-pptx-wasm') {
        return `export const PptxViewer = class { constructor() { throw new Error('PPTX WASM not available on mobile'); } };`;
      }
    },
  };
}

export default defineConfig({
  plugins: [sveltekit(), emptyWasmPlugin()],
  // Mobile dev path; Tauri overrides `devUrl` via tauri.conf.json on android.
  server: { port: 1421, strictPort: true, host: '0.0.0.0' },
  clearScreen: false,
  build: {
    rollupOptions: {
      external: [/\.wasm$/],
    },
  },
  // Prevent vite from resolving imports to the crates directory (wasm plugins)
  resolve: {
    alias: {
      '../../../../crates/fmt-office-universal/pkg/viewit_fmt_office_universal.js': 'virtual:empty-office-wasm',
      '../../../../crates/fmt-archive-universal/pkg/viewit_fmt_archive_universal.js': 'virtual:empty-archive-wasm',
      '@silurus/ooxml/pptx': 'virtual:empty-pptx-wasm',
    },
  },
});
