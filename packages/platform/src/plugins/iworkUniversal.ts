import type { Document, Format } from "../index";

let wasmModule: {
  init: (path?: string | URL | Request | BufferSource) => Promise<void>;
  render: (bytes: Uint8Array, ext: string) => string;
  supported_formats: () => string[];
  name: () => string;
  version: () => string;
} | null = null;

async function loadWasm(): Promise<void> {
  if (wasmModule) return;
  const mod = await import("../../../../crates/fmt-iwork/pkg/viewit_fmt_iwork.js");
  await mod.default();
  wasmModule = mod as any;
}

export async function openIwork(bytes: Uint8Array, name: string, ext: string): Promise<Document> {
  await loadWasm();
  if (!wasmModule) throw new Error("iWork WASM not loaded");
  const json = wasmModule.render(bytes, ext.toLowerCase());
  return JSON.parse(json) as Document;
}

export function isIworkExt(ext: string): boolean {
  return ["pages", "numbers", "key"].includes(ext.toLowerCase());
}

export function extToIworkFormat(ext: string): Format | "unsupported" {
  switch (ext.toLowerCase()) {
    case "pages":
      return "iwork-pages";
    case "numbers":
      return "iwork-numbers";
    case "key":
      return "iwork-key";
    default:
      return "unsupported";
  }
}
