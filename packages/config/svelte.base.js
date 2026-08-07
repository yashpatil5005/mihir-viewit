import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath, URL } from "node:url";

const toSrc = (p) => fileURLToPath(new URL(p, import.meta.url));

/**
 * Shared SvelteKit config bits for the ViewIt apps. Returns a config object
 * (`preprocess` + `kit.alias`) that each app's `svelte.config.js` extends with
 * its own target-specific `kit` additions (`adapter`, `appDir`, ...).
 *
 * @param {object} [extra] Per-app additions, e.g. `{ adapter, appDir }`.
 */
export function viewitKitConfig(extra = {}) {
  const { adapter, appDir, ...rest } = extra;
  const kit = {
    alias: {
      "$viewit-ui": toSrc("../ui/src"),
      "$viewit-platform": toSrc("../platform/src"),
      ...(rest.alias ?? {}),
    },
  };
  if (adapter !== undefined) kit.adapter = adapter({ strict: true });
  if (appDir !== undefined) kit.appDir = appDir;
  return {
    preprocess: vitePreprocess(),
    kit: { ...kit, ...rest },
  };
}
