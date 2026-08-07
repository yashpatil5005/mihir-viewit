import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import { viewitAliases } from "@viewit/config/vite";

function emptyWasmPlugin() {
  return {
    name: "empty-wasm",
    resolveId(id) {
      if (
        id === "virtual:empty-office-wasm" ||
        id === "virtual:empty-archive-wasm" ||
        id === "virtual:empty-pptx-wasm"
      ) {
        return id;
      }
    },
    load(id) {
      if (id === "virtual:empty-office-wasm") {
        return `export default async function() { throw new Error('Office WASM not available on mobile'); }`;
      }
      if (id === "virtual:empty-archive-wasm") {
        return `export default async function() { throw new Error('Archive WASM not available on mobile'); }`;
      }
      if (id === "virtual:empty-pptx-wasm") {
        return `export const PptxViewer = class { constructor() { throw new Error('PPTX WASM not available on mobile'); } };`;
      }
    },
  };
}

export default defineConfig({
  plugins: [sveltekit(), emptyWasmPlugin()],
  // Mobile dev path; Tauri overrides `devUrl` via tauri.conf.json on android.
  server: { port: 1421, strictPort: true, host: "0.0.0.0" },
  clearScreen: false,
  build: {
    rollupOptions: {
      external: [/\.wasm$/],
    },
  },
  // Prevent vite from resolving imports to the crates directory (wasm plugins)
  resolve: {
    alias: {
      ...viewitAliases(),
      "../../../../crates/fmt-office/pkg/viewit_fmt_office.js": "virtual:empty-office-wasm",
      "../../../../crates/fmt-archive/pkg/viewit_fmt_archive.js": "virtual:empty-archive-wasm",
      "@silurus/ooxml/pptx": "virtual:empty-pptx-wasm",
    },
  },
});
