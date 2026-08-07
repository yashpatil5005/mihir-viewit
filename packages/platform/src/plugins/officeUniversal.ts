import type { Document, Format } from "../index";

const OFFICE_EXTS = new Set([
  "docx",
  "docm",
  "dotx",
  "dotm",
  "xlsx",
  "xlsm",
  "xlsb",
  "xls",
  "pptx",
  "pptm",
  "potx",
  "odt",
  "ott",
  "ods",
  "odp",
  "doc",
  "ppt",
]);

export type OfficeUniversalRenderResult =
  | { kind: "docx"; blocks: DocxBlock[] }
  | { kind: "xlsx"; sheets: XlsxSheet[] }
  | { kind: "pptx"; slides: PptxSlide[] }
  | { kind: "odt"; content: string; meta?: Record<string, unknown> }
  | { kind: "ods"; sheets: string[] }
  | { kind: "odp"; slides: string[] }
  | { kind: "legacy"; text: string }
  | { kind: "unsupported"; reason: string };

export interface DocxBlock {
  type: "paragraph" | "table" | "image" | "header" | "footer" | "footnote" | "endnote";
  content: string;
  style?: string;
  metadata?: Record<string, unknown>;
}

export interface XlsxSheet {
  name: string;
  rows: (string | number | null)[][];
  merged_cells?: string[];
  preview_formulas?: string[][];
}

export interface PptxSlide {
  index: number;
  shapes: PptxShape[];
  notes?: string;
}

export interface PptxShape {
  type: "text" | "image" | "table" | "shape" | "chart" | "group";
  content?: string;
  text?: string;
  position?: { x: number; y: number; w: number; h: number };
  metadata?: Record<string, unknown>;
}

let wasmModule: {
  init: (path?: string | URL | Request | BufferSource) => Promise<void>;
  render: (bytes: Uint8Array, ext: string) => string;
  supported_formats: () => string[];
  name: () => string;
  version: () => string;
} | null = null;

async function loadWasm(): Promise<void> {
  if (wasmModule) return;
  try {
    const mod =
      await import("../../../../crates/fmt-office-universal/pkg/viewit_fmt_office_universal.js");
    await mod.default();
    wasmModule = mod as any;
  } catch (e) {
    console.error("[office-universal] Failed to load WASM:", e);
    throw new Error("Office WASM module failed to load");
  }
}

export async function renderOffice(
  bytes: Uint8Array,
  ext: string,
): Promise<OfficeUniversalRenderResult> {
  await loadWasm();
  if (!wasmModule) throw new Error("WASM not loaded");
  const json = wasmModule.render(bytes, ext.toLowerCase());
  return JSON.parse(json) as OfficeUniversalRenderResult;
}

export function isOfficeExt(ext: string): boolean {
  return OFFICE_EXTS.has(ext.toLowerCase());
}

export function extToOfficeFormat(ext: string): Format | "unsupported" {
  switch (ext.toLowerCase()) {
    case "docx":
      return "docx";
    case "docm":
      return "docm";
    case "dotx":
      return "dotx";
    case "dotm":
      return "dotm";
    case "xlsx":
      return "xlsx";
    case "xlsm":
      return "xlsm";
    case "xlsb":
      return "xlsb";
    case "xls":
      return "xls";
    case "pptx":
      return "pptx";
    case "pptm":
      return "pptm";
    case "potx":
      return "potx";
    case "odt":
      return "odt";
    case "ott":
      return "odt";
    case "ods":
      return "ods";
    case "odp":
      return "odp";
    case "doc":
      return "doc";
    case "ppt":
      return "ppt";
    default:
      return "unsupported";
  }
}

export async function openOffice(bytes: Uint8Array, name: string, ext: string): Promise<Document> {
  const result = await renderOffice(bytes, ext);
  const format = extToOfficeFormat(ext);

  if (result.kind === "unsupported") {
    return {
      kind: "unsupported",
      format,
      reason: result.reason,
      suggestion: "open-with-external",
    };
  }

  if (result.kind === "docx") {
    return {
      kind: "docx",
      blocks: result.blocks,
      name,
      byte_len: bytes.length,
      format,
    };
  }

  if (result.kind === "xlsx") {
    return {
      kind: "xlsx",
      sheets: result.sheets,
      name,
      byte_len: bytes.length,
      format,
    };
  }

  if (result.kind === "pptx") {
    return {
      kind: "pptx",
      slides: result.slides,
      name,
      byte_len: bytes.length,
      format,
    };
  }

  if (result.kind === "odt") {
    return {
      kind: "odt",
      content: result.content,
      meta: result.meta,
      name,
      byte_len: bytes.length,
      format,
    };
  }

  if (result.kind === "ods") {
    return {
      kind: "ods",
      sheets: result.sheets,
      name,
      byte_len: bytes.length,
      format,
    };
  }

  if (result.kind === "odp") {
    return {
      kind: "odp",
      slides: result.slides,
      name,
      byte_len: bytes.length,
      format,
    };
  }

  if (result.kind === "legacy") {
    return {
      kind: "placeholder",
      format,
      name,
      byte_len: bytes.length,
      legacy_text: result.text,
    };
  }

  return {
    kind: "unsupported",
    format,
    reason: "Unknown result from WASM",
    suggestion: "open-with-external",
  };
}

export function getSupportedFormats(): string[] {
  if (!wasmModule) return Array.from(OFFICE_EXTS);
  return wasmModule.supported_formats();
}

export function getPluginName(): string {
  if (!wasmModule) return "office-universal";
  return wasmModule.name();
}

export function getPluginVersion(): string {
  if (!wasmModule) return "0.1.0";
  return wasmModule.version();
}
