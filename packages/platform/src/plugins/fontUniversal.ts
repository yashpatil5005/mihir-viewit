import type { Document } from "../index";

let wasmModule: {
  init: (path?: string | URL | Request | BufferSource) => Promise<void>;
  render: (bytes: Uint8Array, ext: string) => string;
  supported_formats: () => string[];
  name: () => string;
  version: () => string;
} | null = null;

async function loadWasm(): Promise<void> {
  if (wasmModule) return;
  const mod = await import("../../../../crates/fmt-font/pkg/viewit_fmt_font.js");
  await mod.default();
  wasmModule = mod as any;
}

export async function openFont(bytes: Uint8Array, name: string, ext: string): Promise<Document> {
  await loadWasm();
  if (!wasmModule) throw new Error("Font WASM not loaded");
  const json = wasmModule.render(bytes, ext.toLowerCase());
  return JSON.parse(json) as Document;
}

export function isFontExt(ext: string): boolean {
  return ["ttf", "otf", "woff", "woff2", "ttc", "pfb", "cff", "dfont", "sfd", "ps"].includes(
    ext.toLowerCase(),
  );
}
