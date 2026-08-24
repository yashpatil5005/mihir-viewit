// Browse mode plumbing: real directory listing on Tauri hosts plus the
// Android "All files access" permission flow through the AndroidBridge JS
// interface (declared in MainActivity.kt).

import { debugLog } from "./debugLog";

const IS_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in (window ?? {});

type AndroidBridgeHost = {
  AndroidBridge?: {
    hasAllFilesAccess?: () => boolean;
    requestAllFilesAccess?: () => void;
    listDir?: (path: string, offset?: number, limit?: number) => string;
  };
};

export function androidBridgeAvailable(): boolean {
  return Boolean((window as unknown as AndroidBridgeHost).AndroidBridge?.listDir);
}

export function hasAllFilesAccess(): boolean {
  const bridge = (window as unknown as AndroidBridgeHost).AndroidBridge;
  return bridge?.hasAllFilesAccess ? bridge.hasAllFilesAccess() : true;
}

export function requestAllFilesAccess(): void {
  const bridge = (window as unknown as AndroidBridgeHost).AndroidBridge;
  bridge?.requestAllFilesAccess?.();
}

export interface BrowseEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
}

export interface BrowseListing {
  path: string;
  parent: string;
  entries: BrowseEntry[];
  total?: number;
  hasMore?: boolean;
}

/**
 * Friendly display names for storage roots. `/sdcard` and
 * `/storage/emulated/0` are the emulated INTERNAL storage — never call them
 * "sdcard" in the UI. Removable cards surface as `/storage/XXXX-XXXX`.
 */
const STORAGE_ROOT_LABELS: Array<{ prefix: string; label: string }> = [
  { prefix: "/storage/emulated/0", label: "Internal storage" },
  { prefix: "/sdcard", label: "Internal storage" },
  { prefix: "/mnt/sdcard", label: "Internal storage" },
  { prefix: "/mnt/shell/emulated", label: "Internal storage" },
];

export interface Crumb {
  label: string;
  path: string;
}

/** Build human-labeled breadcrumb segments for a filesystem path. */
export function friendlyCrumbs(path: string): Crumb[] {
  const normalized = path.replace(/\/+$/, "");
  let rootLabel: string | null = null;
  let rest = normalized;
  for (const root of STORAGE_ROOT_LABELS) {
    if (normalized === root.prefix || normalized.startsWith(`${root.prefix}/`)) {
      rootLabel = root.label;
      rest = normalized.slice(root.prefix.length);
      break;
    }
  }
  if (rootLabel === null) {
    const removable = normalized.match(/^\/storage\/([0-9A-Fa-f]{4}-[0-9A-Fa-f]{4})(\/|$)/);
    if (removable) {
      rootLabel = "SD card";
      rest = normalized.slice(`/storage/${removable[1]}`.length);
    }
  }
  const crumbs: Crumb[] = [];
  if (rootLabel !== null) {
    crumbs.push({
      label: rootLabel,
      path: normalized.slice(0, normalized.length - rest.length) || "/",
    });
  }
  let acc = normalized.slice(0, normalized.length - rest.length);
  for (const part of rest.split("/").filter(Boolean)) {
    acc = `${acc}/${part}`.replace(/^\/\//, "/");
    crumbs.push({ label: part, path: acc });
  }
  return crumbs.length > 0 ? crumbs : [{ label: normalized || "/", path: normalized }];
}

/**
 * Sync asset-protocol URL for a local file (thumbnails, posters). Returns
 * null before `warmFileSrc()` resolves and on the plain web root.
 */
let convertFn: ((path: string) => string) | null | undefined;

export function fileSrcUrl(path: string): string | null {
  if (!IS_TAURI || !convertFn) return null;
  return convertFn(path);
}

export async function warmFileSrc(): Promise<void> {
  if (convertFn !== undefined) return;
  if (!IS_TAURI) {
    convertFn = null;
    return;
  }
  try {
    const { convertFileSrc } = await import("@tauri-apps/api/core");
    convertFn = convertFileSrc;
  } catch {
    convertFn = null;
  }
}

/**
 * Tell the Android side whether the web layer can consume a back press.
 * The native OnBackPressedCallback checks this flag synchronously when the
 * user presses back/gesture, then invokes `window.__viewitConsumeBack()`.
 */
export function reportBackConsumer(active: boolean): void {
  const bridge = (window as unknown as AndroidBridgeHost).AndroidBridge as unknown as {
    reportBackConsumer?: (active: boolean) => void;
  };
  bridge?.reportBackConsumer?.(active);
}

/**
 * List one directory page via the native side. Returns null when
 * unsupported. Pagination keeps huge folders from OOMing the webview.
 */
export async function browseDir(path = "", offset = 0, limit = 300): Promise<BrowseListing | null> {
  if (!IS_TAURI) return null;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<BrowseListing>("browse_dir", {
      path: path || null,
      offset,
      limit,
    });
  } catch {
    return null;
  }
}

/**
 * List a directory on Android through the WebView JS bridge (synchronous,
 * permission-gated). Returns null when the bridge is unavailable or denied.
 */
export interface FuzzyFileMatch {
  name: string;
  path: string;
  isDir: boolean;
  score: number;
}

export interface ContentSearchMatch {
  path: string;
  lineNumber: number;
  lineText: string;
}

export async function searchFilesFuzzy(
  query: string,
  root?: string,
  limit = 50,
): Promise<FuzzyFileMatch[]> {
  if (!IS_TAURI) return [];
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<FuzzyFileMatch[]>("search_files_fuzzy", {
      query,
      root: root || null,
      limit,
    });
  } catch {
    return [];
  }
}

export async function searchFilesContent(
  query: string,
  root?: string,
  limit = 50,
): Promise<ContentSearchMatch[]> {
  if (!IS_TAURI) return [];
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<ContentSearchMatch[]>("search_files_content", {
      query,
      root: root || null,
      limit,
    });
  } catch {
    return [];
  }
}

export function androidListDir(path = "", offset = 0, limit = 300): BrowseListing | null {
  const bridge = (window as unknown as AndroidBridgeHost).AndroidBridge;
  if (!bridge?.listDir || !bridge.hasAllFilesAccess?.()) return null;
  try {
    const raw =
      offset === 0 && limit === 300 ? bridge.listDir(path) : bridge.listDir(path, offset, limit);
    const parsed = JSON.parse(raw) as
      | {
          ok: true;
          path: string;
          parent: string;
          entries: BrowseEntry[];
          total?: number;
          hasMore?: boolean;
        }
      | { ok: false; error: string };
    return parsed.ok ? parsed : null;
  } catch {
    return null;
  }
}
