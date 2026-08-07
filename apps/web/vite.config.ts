import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import { viewitAliases, wasmPlugin } from "@viewit/config/vite";

export default defineConfig({
  plugins: [wasmPlugin(), sveltekit()],
  resolve: { alias: viewitAliases() },
  server: { port: 1422, strictPort: true },
});
