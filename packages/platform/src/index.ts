// @viewit/platform — single seam between the Svelte frontend and the
// backing implementation (Tauri Rust commands OR direct WASM calls).
//
// Per plan §2 ("platform.ts abstraction") + §7 ("swap invoke() (Tauri)
// for direct WASM function call (web) behind one interface").
//
// Phase 1 implements the Tauri host path. The web/WASM path is implemented
// in Phase 1.6 of the scaffold work; until then, the web path falls back to
// a JS-only text reader so the loop is exercisable immediately.

export type DocumentKind = 'text' | 'unsupported' | 'placeholder';
export type Format =
  | 'plain-text' | 'markdown' | 'json' | 'csv' | 'code'
  | 'pdf' | 'image-png' | 'image-jpg' | 'image-webp' | 'image-gif'
  | 'image-bmp' | 'image-tiff' | 'image-svg'
  | 'epub'
  | 'archive-zip' | 'archive-tar' | 'archive-tar-gz' | 'archive-7z' | 'archive-rar'
  | 'docx' | 'xlsx' | 'pptx' | 'odt' | 'ods' | 'odp' | 'rtf'
  | 'iwork-pages' | 'iwork-numbers' | 'iwork-key'
  | 'unsupported';

export type Suggestion = 'open-with-external' | 'none';

export interface Document {
  kind: DocumentKind;
  // Discriminated union payload:
  [k: string]: unknown;
}

export interface TextDocument extends Document {
  kind: 'text';
  content: string;
  encoding: string;
  byte_len: number;
}

export interface UnsupportedDocument extends Document {
  kind: 'unsupported';
  format: Format;
  reason: string;
  suggestion: Suggestion;
}

export interface PlaceholderDocument extends Document {
  kind: 'placeholder';
  format: Format;
  name: string;
  byte_len: number;
}

// ---------------------------------------------------------------------------
// Detect: are we in a Tauri host or a plain browser?
// ---------------------------------------------------------------------------
const IS_TAURI =
  typeof window !== 'undefined' &&
  // Tauri v2 injects __TAURI_INTERNALS__ into the webview.
  '__TAURI_INTERNALS__' in (window ?? {});

// ---------------------------------------------------------------------------
// API surface
// ---------------------------------------------------------------------------

/** Returns a list of file URIs opened during cold start (may be empty). */
export async function openedFiles(): Promise<string[]> {
  if (!IS_TAURI) return [];
  const { invoke } = await import('@tauri-apps/api/core');
  return invoking(async () => invoke<string[]>('opened_urls'));
}

/** Subscribe to file-opened events delivered while the app is running. */
export async function onOpenedFiles(cb: (urls: string[]) => void): Promise<() => void> {
  if (!IS_TAURI) return () => {};
  const { listen } = await import('@tauri-apps/api/event');
  const unlisten = await listen<string[]>('opened', (e) => cb(e.payload));
  return unlisten;
}

/**
 * Open a file URI and return a structured Document. This is the main
 * entry-point the Viewer root component calls.
 */
export async function openFile(uri: string): Promise<Document> {
  if (IS_TAURI) {
    return await invokeOpenUri(uri);
  } else {
    // Web fallback — pure JS, ignores most formats but renders text.
    return await webOpenFile(uri);
  }
}

/** Per ADR 0004 — RAR5 / unsupported with OpenWith-External suggestion. */
export async function openWithExternal(uri: string): Promise<void> {
  if (!IS_TAURI) {
    window.open(uri, '_blank');
    return;
  }
  const { invoke } = await import('@tauri-apps/api/core');
  // The desktop/mobile `tauri-plugin-shell` exposes an open() for OS handlers.
  await invoke('plugin:shell|open', { uri });
}

// ---------------------------------------------------------------------------
// Internal: Tauri invoke adapter
// ---------------------------------------------------------------------------
async function invokeOpenUri(uri: string): Promise<Document> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoking(async () => {
    const doc = await invoke<Document>('open_uri', { uri });
    return doc;
  });
}

async function invoking<T>(fn: () => Promise<T>): Promise<T> {
  try {
    return await fn();
  } catch (e: any) {
    // Tauri error responses come as { message } style strings; surface as-is.
    throw new Error(typeof e === 'string' ? e : (e?.message ?? String(e)));
  }
}

// ---------------------------------------------------------------------------
// Internal: web-only fallback reader (no Tauri, no WASM yet).
// ---------------------------------------------------------------------------
async function webOpenFile(uri: string): Promise<Document> {
  // For Phase 1 web path: fetch as bytes, detect MIME, render text shapes.
  // The real Phase 1.6 will replace this with a direct call into the
  // WASM-compiled viewit-core crate, exactly as the Tauri host does.
  const res = await fetch(uri);
  if (!res.ok) {
    throw new Error(`Failed to read ${uri}: ${res.status} ${res.statusText}`);
  }
  const buf = new Uint8Array(await res.arrayBuffer());
  const name = uri.split('/').pop() ?? 'file';
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  if (isTextExt(ext)) {
    const lossy = new TextDecoder('utf-8', { fatal: false }).decode(buf);
    return {
      kind: 'text',
      content: lossy,
      encoding: 'utf-8',
      byte_len: buf.length
    };
  }
  return {
    kind: 'placeholder',
    format: extToFormat(ext),
    name,
    byte_len: buf.length
  };
}

const TEXT_EXT = new Set(['txt', 'text', 'log', 'md', 'markdown', 'json', 'csv', 'tsv', 'jsonl']);
function isTextExt(ext: string): boolean {
  return TEXT_EXT.has(ext);
}

function extToFormat(ext: string): Format {
  switch (ext) {
    case 'png': return 'image-png';
    case 'jpg':
    case 'jpeg': return 'image-jpg';
    case 'webp': return 'image-webp';
    case 'gif': return 'image-gif';
    case 'bmp': return 'image-bmp';
    case 'tif':
    case 'tiff': return 'image-tiff';
    case 'svg': return 'image-svg';
    case 'pdf': return 'pdf';
    case 'epub': return 'epub';
    case 'zip': return 'archive-zip';
    case 'tar': return 'archive-tar';
    case 'tgz':
    case 'gz': return 'archive-tar-gz';
    case '7z': return 'archive-7z';
    case 'rar': return 'archive-rar';
    case 'docx': return 'docx';
    case 'xlsx': return 'xlsx';
    case 'pptx': return 'pptx';
    case 'odt': return 'odt';
    case 'ods': return 'ods';
    case 'odp': return 'odp';
    case 'rtf': return 'rtf';
    case 'pages': return 'iwork-pages';
    case 'numbers': return 'iwork-numbers';
    case 'key': return 'iwork-key';
    default: return 'unsupported';
  }
}
