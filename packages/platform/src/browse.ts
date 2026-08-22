// Browse mode plumbing: real directory listing on Tauri hosts plus the
// Android "All files access" permission flow through the AndroidBridge JS
// interface (declared in MainActivity.kt).

import { debugLog } from "./debugLog";

const IS_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in (window ?? {});

type AndroidBridgeHost = {
  AndroidBridge?: {
    hasAllFilesAccess?: () => boolean;
    requestAllFilesAccess?: () => void;
    listDir?: (path: string) => string;
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
}

/** List a directory via the native side. Returns null when unsupported. */
export async function browseDir(path = ""): Promise<BrowseListing | null> {
  if (!IS_TAURI) return null;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<BrowseListing>("browse_dir", { path: path || null });
  } catch {
    return null;
  }
}

/**
 * List a directory on Android through the WebView JS bridge (synchronous,
 * permission-gated). Returns null when the bridge is unavailable or denied.
 */
export function androidListDir(path = ""): BrowseListing | null {
  const bridge = (window as unknown as AndroidBridgeHost).AndroidBridge;
  if (!bridge?.listDir || !bridge.hasAllFilesAccess?.()) return null;
  try {
    const parsed = JSON.parse(bridge.listDir(path)) as
      | { ok: true; path: string; parent: string; entries: BrowseEntry[] }
      | { ok: false; error: string };
    return parsed.ok ? parsed : null;
  } catch {
    return null;
  }
}
