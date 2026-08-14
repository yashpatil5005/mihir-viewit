import type { Document, Format } from "../index";

const ARCHIVE_EXTS = new Set([
  "zip",
  "tar",
  "tgz",
  "gz",
  "tar.gz",
  "tar.bz2",
  "tbz2",
  "tar.xz",
  "txz",
  "tar.zst",
  "tzst",
  "tar.lz4",
  "tar.lzma",
  "tlz",
  "bz2",
  "xz",
  "zst",
  "lz4",
  "lzma",
  "7z",
  "rar",
]);

export interface ArchiveEntry {
  name: string;
  size: number;
  compressed_size: number;
  is_dir: boolean;
}

export interface ArchiveManifest {
  entries: ArchiveEntry[];
  format: string;
}

let wasmModule: {
  init: (path?: string | URL | Request | BufferSource) => Promise<void>;
  parseArchive: (bytes: Uint8Array, filename: string) => string;
  extractEntry: (bytes: Uint8Array, filename: string, entryName: string) => Uint8Array;
  extractAll: (bytes: Uint8Array, filename: string) => string;
  supportedFormats: () => string[];
  name: () => string;
  version: () => string;
} | null = null;

async function loadWasm(): Promise<void> {
  if (wasmModule) return;
  try {
    const mod = await import("../../../../crates/fmt-archive/pkg/viewit_fmt_archive.js");
    await mod.default();
    wasmModule = mod as any;
  } catch (e) {
    console.error("[archive-universal] Failed to load WASM:", e);
    throw new Error("Archive WASM module failed to load");
  }
}

export async function parseArchive(bytes: Uint8Array, filename: string): Promise<ArchiveManifest> {
  await loadWasm();
  if (!wasmModule) throw new Error("WASM not loaded");
  const json = wasmModule.parseArchive(bytes, filename);
  return JSON.parse(json) as ArchiveManifest;
}

export async function extractEntry(
  bytes: Uint8Array,
  filename: string,
  entryName: string,
): Promise<Uint8Array> {
  await loadWasm();
  if (!wasmModule) throw new Error("WASM not loaded");
  return wasmModule.extractEntry(bytes, filename, entryName);
}

export async function extractAll(
  bytes: Uint8Array,
  filename: string,
): Promise<[string, Uint8Array][]> {
  await loadWasm();
  if (!wasmModule) throw new Error("WASM not loaded");
  const json = wasmModule.extractAll(bytes, filename);
  const entries = JSON.parse(json) as [string, number[]][];
  return entries.map(([name, data]) => [name, new Uint8Array(data)]);
}

export function isArchiveExt(ext: string): boolean {
  return ARCHIVE_EXTS.has(ext.toLowerCase());
}

export function extToArchiveFormat(ext: string): Format | "unsupported" {
  switch (ext.toLowerCase()) {
    case "zip":
      return "archive-zip";
    case "tar":
      return "archive-tar";
    case "tgz":
    case "gz":
      return "archive-tar-gz";
    case "7z":
      return "archive-7z";
    case "rar":
      return "archive-rar";
    default:
      return "unsupported";
  }
}

export async function openArchive(bytes: Uint8Array, name: string, ext: string): Promise<Document> {
  const format = extToArchiveFormat(ext);
  if (format === "unsupported") {
    return {
      kind: "unsupported",
      format: "unsupported",
      reason: `Archive format .${ext} not supported`,
      suggestion: "open-with-external",
    };
  }

  try {
    const manifest = await parseArchive(bytes, name);
    return {
      kind: "archive",
      entries: manifest.entries,
      format: manifest.format as Format,
      byte_len: bytes.length,
      name,
    };
  } catch (e) {
    console.error("[archive-universal] parse failed:", e);
    return {
      kind: "unsupported",
      format,
      reason: `Archive parsing failed: ${e instanceof Error ? e.message : String(e)}`,
      suggestion: "open-with-external",
    };
  }
}
