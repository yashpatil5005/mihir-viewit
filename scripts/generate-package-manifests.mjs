#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { mkdirSync } from "node:fs";
import { parsePluginPackageManifestV1, parseProviderDescriptorV1 } from "../packages/contracts/src/index.ts";

const root = resolve(import.meta.dirname, "..");
const providers = JSON.parse(readFileSync(resolve(root, "packages/contracts/catalog/providers.v1.json"), "utf8")).map(parseProviderDescriptorV1);
const sources = {
  "office-universal": "apps/mobile/plugins/office-universal/plugin.json",
  "pptx-vanilla": "apps/mobile/plugins/pptx-vanilla/plugin.json",
  "compression-universal": "apps/mobile/plugins/compression-universal/plugin.json",
  "font-universal": "apps/mobile/plugins/font-universal/plugin.json",
  "iwork-universal": "apps/mobile/plugins/iwork-universal/plugin.json",
  "player-base": "plugins/player-base/plugin.json",
  "editor-base": "plugins/editor-base/plugin.json",
  "ffmpeg-transcoder": "plugins/ffmpeg-transcoder/plugin.json",
};

const runtime = (value) => {
  if (value === "js") return "webview-js";
  if (value === "android-dex-jni") return value;
  return "android-dex-jni";
};
const outputs = [];
for (const [packageId, sourcePath] of Object.entries(sources)) {
  const source = JSON.parse(readFileSync(resolve(root, sourcePath), "utf8"));
  const manifest = {
    schemaVersion: 1,
    id: packageId,
    name: source.name,
    version: source.version,
    description: source.description ?? "",
    publisher: "viewit",
    minAppVersion: source.minAppVersion ?? 1,
    runtime: runtime(source.runtime),
    ...(source.entryClass ? { entryClass: source.entryClass } : {}),
    ...(source.jsEntry ? { jsEntry: source.jsEntry } : {}),
    ...(source.cssEntry ? { cssEntry: source.cssEntry } : {}),
    ...(source.abi ? { abi: source.abi } : {}),
    abiVersion: source.abiVersion ?? 1,
    providers: providers.filter((provider) => provider.packageId === packageId),
  };
  parsePluginPackageManifestV1(manifest);
  outputs.push([
    resolve(root, `packages/contracts/manifests/${packageId}.v1.json`),
    JSON.stringify(manifest, null, 2) + "\n",
  ]);
}

const check = process.argv.includes("--check");
let changed = false;
for (const [path, content] of outputs) {
  const current = (() => { try { return readFileSync(path, "utf8"); } catch { return ""; } })();
  if (current === content) continue;
  changed = true;
  if (!check) {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, content);
  }
  console.error(`[contracts] ${check ? "stale" : "generated"}: ${path.replace(root + "/", "")}`);
}
if (check && changed) process.exit(1);
console.log(`[contracts] ${outputs.length} canonical package manifests`);
