import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import { viewitAliases } from "@viewit/config/vite";

export default defineConfig({
  plugins: [sveltekit()],
  resolve: { alias: viewitAliases() },
  server: { port: 1420, strictPort: true },
  clearScreen: false,
});
