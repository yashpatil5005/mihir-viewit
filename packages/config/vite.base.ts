import { fileURLToPath, URL } from "node:url";
import type { PluginOption } from "vite";
import wasm from "vite-plugin-wasm";

const toSrc = (p: string) => fileURLToPath(new URL(p, import.meta.url));

const UI_SRC = toSrc("../ui/src");
const PLATFORM_SRC = toSrc("../platform/src");

export function viewitAliases(): Record<string, string> {
  return {
    "@viewit/ui": UI_SRC,
    "@viewit/platform": PLATFORM_SRC,
    "$viewit-ui": UI_SRC,
    "$viewit-platform": PLATFORM_SRC,
  };
}

export function wasmPlugin(): PluginOption {
  return wasm();
}
