import { mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const temporary = resolve(root, "build", "tmp");
mkdirSync(temporary, { recursive: true });
const result = spawnSync(
  process.execPath,
  [resolve(root, "node_modules", "vitest", "vitest.mjs"), "run", "-c", "packages/platform/vitest.config.ts"],
  {
    cwd: root,
    env: { ...process.env, TMPDIR: temporary, TMP: temporary, TEMP: temporary },
    stdio: "inherit",
  },
);
process.exit(result.status ?? 1);
