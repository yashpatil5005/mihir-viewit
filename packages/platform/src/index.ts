import {
  OPEN_BYTES_CAP,
  checkFileBeforeRead,
  unsupportedDocument,
  fileExtension,
} from './filePolicy';
import { displayNameFromUri } from './displayNameFromUri';

// @viewit/platform — single seam between the Svelte frontend and the
// backing implementation (Tauri Rust commands OR direct WASM calls).
//
// Per plan §2 ("platform.ts abstraction") + §7 ("swap invoke() (Tauri)
// for direct WASM function call (web) behind one interface").
//
// Phase 1 implements the Tauri host path. The web/WASM path is implemented
// in Phase 1.6 of the scaffold work; until then, the web path falls back to
// a JS-only text reader so the loop is exercisable immediately.

export type DocumentKind =
  | 'text' | 'image' | 'markdown' | 'json' | 'csv' | 'pdf' | 'epub' | 'mobi' | 'azw3'
  | 'fictionbook' | 'palmdoc' | 'archive' | 'pptx' | 'docx' | 'xlsx'
  | 'media' | 'stream-file' | 'unsupported' | 'placeholder' | 'font';
export type Format =
  | 'plain-text' | 'markdown' | 'json' | 'csv' | 'code'
  | 'pdf' | 'image-png' | 'image-jpg' | 'image-webp' | 'image-gif'
  | 'image-bmp' | 'image-tiff' | 'image-svg' | 'image-raw' | 'image-heic' | 'image-psd'
  | 'epub' | 'mobi' | 'azw3' | 'fictionbook' | 'palmdoc'
  | 'archive-zip' | 'archive-tar' | 'archive-tar-gz' | 'archive-7z' | 'archive-rar'
  | 'docx' | 'docm' | 'dotx' | 'dotm'
  | 'xlsx' | 'xlsm' | 'xlsb' | 'xls'
  | 'pptx' | 'pptm' | 'potx'
  | 'odt' | 'ott' | 'ods' | 'odp' | 'doc' | 'ppt' | 'rtf'
  | 'plist' | 'ics' | 'vcf'
  | 'video' | 'audio'
  | 'iwork-pages' | 'iwork-numbers' | 'iwork-key'
  | 'font'
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
export { IS_TAURI };

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
    const unlisten = await listen<string[] | string>('opened', (e) => {
      const p = e.payload;
      const urls = Array.isArray(p) ? p : [p];
      cb(urls);
    });
  return unlisten;
}

/**
 * Open a file URI and return a structured Document. This is the main
 * entry-point the Viewer root component calls.
 */
export async function openFile(uri: string): Promise<Document> {
  if (uri.startsWith('blob:')) {
    throw new Error(
      'blob: URLs cannot be read by the host. Use openFileFromPicker(File) after the in-app file picker.',
    );
  }
  if (IS_TAURI) {
    const { invoke } = await import('@tauri-apps/api/core');
    const blocked = await invoking(() =>
      invoke<Document | null>('probe_uri', { uri, name: displayNameFromUri(uri) }),
    );
    if (blocked && typeof blocked === 'object' && (blocked as Document).kind === 'unsupported') {
      return blocked as Document;
    }
    const { debugLog } = await import('./debugLog');
    debugLog(`openFile ${uri.slice(0, 80)}…`);
    try {
      const doc = await invokeOpenUri(uri, displayNameFromUri(uri));
      debugLog(`ok kind=${(doc as Document).kind}`);
      return doc;
    } catch (e) {
      debugLog(`openFile err: ${e instanceof Error ? e.message : String(e)}`);
      throw e;
    }
  }
  return await webOpenFile(uri);
}

export { OPEN_BYTES_CAP, checkFileBeforeRead, unsupportedDocument, fileExtension };
export { assetUrlForPath } from './mediaUrl';
export { readMaterializedBytes } from './readMaterialized';
export { resolvePptxAssetPath } from './pptxAsset';
export { displayNameFromUri } from './displayNameFromUri';
export { pickSingleFile } from './pickFile';
export { debugLog, debugLogLines, debugLogClear } from './debugLog';

/** Register a URI with the stream server and return a stream URL. */
export async function registerStreamUri(uri: string): Promise<string> {
  if (!IS_TAURI) return uri;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoking(() => invoke<string>('register_stream_uri', { uri }));
}

/** After `<input type="file">` — checks size/type first (no read for video/large). */
const STREAM_PICKER_EXT = new Set([
  'pdf',
  // video
  'mp4', 'm4v', 'webm', 'mkv', 'mov', 'avi', 'mpg', 'mpeg', '3gp', 'wmv', 'flv', 'ts',
  'asf', 'f4v', 'hevc', 'm2ts', 'm2v', 'mjpeg', 'mts', 'mxf', 'ogv', 'rm', 'swf', 'vob', 'wtv',
  // audio
  'mp3', 'm4a', 'aac', 'flac', 'ogg', 'wav', 'wma', 'opus',
  '8svx', 'ac3', 'aif', 'aiff', 'amb', 'au', 'avr', 'caf', 'cdda', 'cvs', 'cvsd', 'cvu', 'dts',
  'dvms', 'fap', 'fssd', 'gsrt', 'hcom', 'htk', 'ima', 'ircam', 'm4r', 'maud', 'mp2', 'nist',
  'oga', 'paf', 'prc', 'pvf', 'ra', 'sd2', 'sln', 'smp', 'snd', 'sndr', 'sndt', 'sou', 'sph',
  'spx', 'tta', 'txw', 'vms', 'voc', 'vox', 'w64', 'wv', 'wve',
]);

function pickerStreamDocument(file: File): Document | null {
  const viewitUri = (file as File & { viewitUri?: string }).viewitUri;
  if (viewitUri) return null;
  const ext = fileExtension(file.name);
  if (!STREAM_PICKER_EXT.has(ext)) return null;
  if (IS_TAURI) {
    return null;
  }
  if (file.size <= OPEN_BYTES_CAP) return null;
  return streamDocFromFile(file, ext);
}

const VIDEO_AUDIO = new Set([
  // video
  'mp4', 'm4v', 'webm', 'mkv', 'mov', 'avi', 'mpg', 'mpeg', '3gp', 'wmv', 'flv', 'ts',
  'asf', 'f4v', 'hevc', 'm2ts', 'm2v', 'mjpeg', 'mts', 'mxf', 'ogv', 'rm', 'swf', 'vob', 'wtv',
  // audio
  'mp3', 'm4a', 'aac', 'flac', 'ogg', 'wav', 'wma', 'opus',
  '8svx', 'ac3', 'aif', 'aiff', 'amb', 'au', 'avr', 'caf', 'cdda', 'cvs', 'cvsd', 'cvu', 'dts',
  'dvms', 'fap', 'fssd', 'gsrt', 'hcom', 'htk', 'ima', 'ircam', 'm4r', 'maud', 'mp2', 'nist',
  'oga', 'paf', 'prc', 'pvf', 'ra', 'sd2', 'sln', 'smp', 'snd', 'sndr', 'sndt', 'sou', 'sph',
  'spx', 'tta', 'txw', 'vms', 'voc', 'vox', 'w64', 'wv', 'wve',
]);

const AUDIO_ONLY = new Set([
  'mp3', 'm4a', 'aac', 'flac', 'ogg', 'wav', 'wma', 'opus',
  '8svx', 'ac3', 'aif', 'aiff', 'amb', 'au', 'avr', 'caf', 'cdda', 'cvs', 'cvsd', 'cvu', 'dts',
  'dvms', 'fap', 'fssd', 'gsrt', 'hcom', 'htk', 'ima', 'ircam', 'm4r', 'maud', 'mp2', 'nist',
  'oga', 'paf', 'prc', 'pvf', 'ra', 'sd2', 'sln', 'smp', 'snd', 'sndr', 'sndt', 'sou', 'sph',
  'spx', 'tta', 'txw', 'vms', 'voc', 'vox', 'w64', 'wv', 'wve',
]);

function streamDocFromFile(file: File, ext: string): Document | null {
  const name = file.name || 'file';
  if (ext === 'pdf') {
    return {
      kind: 'pdf',
      native: true,
      pages: [],
      page_count: 0,
      byte_len: file.size,
      name,
    } as Document;
  }
  return {
    kind: 'media',
    media_kind: AUDIO_ONLY.has(ext) ? 'audio' : 'video',
    format: AUDIO_ONLY.has(ext) ? 'audio' : 'video',
    name,
    byte_len: file.size,
  } as Document;
}

export async function openFileFromPicker(file: File): Promise<Document> {
  const viewitUri = (file as File & { viewitUri?: string }).viewitUri;
  if (viewitUri) {
    return openFile(viewitUri);
  }
  const gate = checkFileBeforeRead(file);
  if (gate.reject) {
    return unsupportedDocument(gate.reason, gate.openWithExternal);
  }
  const streamDoc = pickerStreamDocument(file);
  if (streamDoc) return streamDoc;
  const name = file.name || 'file';
  const ext = fileExtension(name);
  if (IS_TAURI && ext === 'pdf') {
    const { invoke } = await import('@tauri-apps/api/core');
    const buf = new Uint8Array(await file.arrayBuffer());
    if (buf.length <= OPEN_BYTES_CAP) {
      let binary = '';
      const chunk = 0x8000;
      for (let i = 0; i < buf.length; i += chunk) {
        binary += String.fromCharCode(...buf.subarray(i, i + chunk));
      }
      const b64 = btoa(binary);
      const doc = await invoking(() => invoke<Document>('open_bytes_b64', { b64, name }));
      if ((doc as { native?: boolean }).native) return doc;
    }
  }
  const buf = new Uint8Array(await file.arrayBuffer());
  if (IS_TAURI) {
    const { invoke } = await import('@tauri-apps/api/core');
    let binary = '';
    const chunk = 0x8000;
    for (let i = 0; i < buf.length; i += chunk) {
      binary += String.fromCharCode(...buf.subarray(i, i + chunk));
    }
    const b64 = btoa(binary);
    return invoking(() => invoke<Document>('open_bytes_b64', { b64, name }));
  }
  return webOpenBytes(buf, name);
}

/** Lazy PDF page (share / open_uri path — re-reads file in Rust). */
export async function pdfPage(uri: string, index: number): Promise<string> {
  if (!IS_TAURI) throw new Error('pdfPage only on Tauri');
  const { invoke } = await import('@tauri-apps/api/core');
  return invoking(() => invoke<string>('pdf_page', { uri, index }));
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
async function invokeOpenUri(uri: string, name?: string | null): Promise<Document> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoking(async () => {
    const doc = await invoke<Document>('open_uri', { uri, name: name ?? null });
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
function webOpenBytes(buf: Uint8Array, name: string): Promise<Document> {
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  const uri = URL.createObjectURL(new Blob([buf]));
  return webOpenFile(uri, { bytes: buf, name, ext }).finally(() => URL.revokeObjectURL(uri));
}

async function webOpenFile(
  uri: string,
  hint?: { bytes: Uint8Array; name: string; ext: string },
): Promise<Document> {
  let buf: Uint8Array;
  if (hint) {
    buf = hint.bytes;
  } else {
    const res = await fetch(uri);
    if (!res.ok) {
      throw new Error(`Failed to read ${uri}: ${res.status} ${res.statusText}`);
    }
    buf = new Uint8Array(await res.arrayBuffer());
  }
  const name = hint?.name ?? uri.split('/').pop()?.split('?')[0] ?? 'file';
  const ext = hint?.ext ?? name.split('.').pop()?.toLowerCase() ?? '';

  // Image — native webview decoder path. Hand the URI directly back to
  // the frontend ImageViewer's <img> tag.
  if (isImageExt(ext)) {
    return {
      kind: 'image',
      byte_len: buf.length,
      name,
      format: extToFormat(ext) as any,
    };
  }

  if (isTextExt(ext)) {
    const lossy = new TextDecoder('utf-8', { fatal: false }).decode(buf);
    // Mirror the Rust routing — markdown/json/csv parsed JS-side as a fallback.
    if (ext === 'md' || ext === 'markdown') {
      // Minimal CommonMark fallback: just escape and wrap in <pre>. The Rust
      // path through pulldown-cmark is the real one; web fills for Phase 1.
      return { kind: 'markdown', html: `<pre>${escapeHtml(lossy)}</pre>`, byte_len: buf.length };
    }
    if (ext === 'json') {
      try {
        const pretty = JSON.stringify(JSON.parse(lossy), null, 2);
        return { kind: 'json', pretty, byte_len: buf.length };
      } catch {
        return { kind: 'json', pretty: lossy, byte_len: buf.length };
      }
    }
    if (ext === 'csv' || ext === 'tsv') {
      // Naive CSV: split first ~50 lines.
      const lines = lossy.split(/\r?\n/).slice(0, 200);
      const sep = ext === 'tsv' ? '\t' : ',';
      const parseRow = (l: string): string[] => l.split(sep).map(s => s.replace(/^"(.*)"$/, '$1'));
      const header = parseRow(lines[0] ?? '');
      const previewRows = lines.slice(1).map(parseRow);
      return { kind: 'csv', header, preview_rows: previewRows, total_rows_hint: previewRows.length, byte_len: buf.length };
    }
    return { kind: 'text', content: lossy, encoding: 'utf-8', byte_len: buf.length };
  }
  return {
    kind: 'placeholder',
    format: extToFormat(ext),
    name,
    byte_len: buf.length,
  };
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"]/g, (c) => ({ '&': '&', '<': '<', '>': '>', '"': '"' }[c] ?? c));
}

const IMAGE_EXT = new Set(['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tif', 'tiff', 'svg', 'heic', 'heif', 'avif', 'psd', 'dng', 'cr2', 'cr3', 'nef', 'arw', 'orf', 'rw2', 'raf', 'srw', 'pef', 'cur', 'dds', 'erf', 'exr', 'fts', 'hdr', 'jp2', 'jpe', 'jps', 'mng', 'nrw', 'pam', 'pbm', 'pcd', 'pcx', 'pes', 'pfm', 'pgm', 'picon', 'pict', 'pnm', 'ppm', 'ras', 'sfw', 'sgi', 'tga', 'wbmp', 'wpg', 'x3f', 'xbm', 'xcf', 'xpm', 'xwd', 'djvu', 'djv']);
function isImageExt(ext: string): boolean {
  return IMAGE_EXT.has(ext);
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
    case 'ico':
    case 'jfif': return 'image-png';
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
    case 'docm': return 'docm';
    case 'dotx': return 'dotx';
    case 'dotm': return 'dotm';
    case 'xlsx': return 'xlsx';
    case 'xlsm': return 'xlsm';
    case 'xlsb': return 'xlsb';
    case 'xls': return 'xls';
    case 'pptx': return 'pptx';
    case 'pptm': return 'pptm';
    case 'potx': return 'potx';
    case 'odt': return 'odt';
    case 'ott': return 'odt';
    case 'ods': return 'ods';
    case 'odp': return 'odp';
    case 'doc': return 'doc';
    case 'ppt': return 'ppt';
    case 'rtf': return 'rtf';
    case 'psd': return 'image-psd';
    case 'heic':
    case 'heif':
    case 'avif': return 'image-heic';
    case 'djvu':
    case 'djv': return 'image-raw';
    case 'dng':
    case 'cr2':
    case 'cr3':
    case 'nef':
    case 'arw':
    case 'orf':
    case 'rw2':
    case 'raf':
    case 'srw':
    case 'pef':
    case 'cur':
    case 'dds':
    case 'erf':
    case 'exr':
    case 'fts':
    case 'hdr':
    case 'jp2':
    case 'jpe':
    case 'jps':
    case 'mng':
    case 'nrw':
    case 'pam':
    case 'pbm':
    case 'pcd':
    case 'pcx':
    case 'pes':
    case 'pfm':
    case 'pgm':
    case 'picon':
    case 'pict':
    case 'pnm':
    case 'ppm':
    case 'ras':
    case 'sfw':
    case 'sgi':
    case 'tga':
    case 'wbmp':
    case 'wpg':
    case 'x3f':
    case 'xbm':
    case 'xcf':
    case 'xpm':
    case 'xwd': return 'image-raw';
    case 'mobi': return 'mobi';
    case 'azw3': return 'azw3';
    case 'fb2': return 'fictionbook';
    case 'lrf':
    case 'pdb':
    case 'snb': return 'palmdoc';
    case 'ttf':
    case 'otf':
    case 'woff':
    case 'woff2':
    case 'pfb':
    case 'cff':
    case 'dfont':
    case 'sfd':
    case 'ps': return 'font';
    case 'pages': return 'iwork-pages';
    case 'numbers': return 'iwork-numbers';
    case 'key': return 'iwork-key';
    default: return 'unsupported';
  }
}
