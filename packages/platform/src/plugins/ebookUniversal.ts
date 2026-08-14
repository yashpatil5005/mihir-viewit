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
  const mod = await import("../../../../crates/fmt-ebook/pkg/viewit_fmt_ebook.js");
  await mod.default();
  wasmModule = mod as any;
}

export async function openEbook(bytes: Uint8Array, name: string, ext: string): Promise<Document> {
  await loadWasm();
  if (!wasmModule) throw new Error("Ebook WASM not loaded");
  const json = wasmModule.render(bytes, ext.toLowerCase());
  const doc = JSON.parse(json) as Document;
  // Normalize to the epub kind so the shared EpubViewer handles it.
  return { ...doc, kind: "epub", format: extToEbookFormat(ext) as Format };
}

export function isEbookExt(ext: string): boolean {
  return ["epub", "mobi", "azw3"].includes(ext.toLowerCase());
}

export function extToEbookFormat(ext: string): Format | "unsupported" {
  switch (ext.toLowerCase()) {
    case "epub":
      return "epub";
    case "mobi":
      return "mobi";
    case "azw3":
      return "azw3";
    default:
      return "unsupported";
  }
}
