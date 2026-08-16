import { providerSupportsFormat, resolveFormatCapability } from "@viewit/platform";
import type { ProviderDescriptorV1 } from "@viewit/contracts";

export interface PluginInfo {
  id: string;
  name: string;
  version: string;
  description: string;
  formats: string[];
  downloadUrl?: string;
  sizeBytes?: number;
  installedSizeBytes?: number;
  checksum?: string;
  abi?: string;
  abiVersion?: number;
  minAppVersion?: number;
  entryClass?: string;
  installed?: boolean;
  installedVersion?: string;
  updateAvailable?: boolean;
  sourceId?: string;
  sourceName?: string;
  sourceUrl?: string;
  /** Plugin runtime: 'native' | 'dex' | 'js'. JS plugins ship a WebView bundle. */
  runtime?: string;
  /** Plugin base the add-on provides: 'view' | 'play' | 'edit' | 'tool'. See docs/PLUGIN-BASES.md. */
  base?: string;
  /** Fine-grained actions provided inside that base (e.g. play, edit, save, extract). */
  capabilities?: string[];
  jsEntry?: string;
  cssEntry?: string;
  /** Documented storage-permission scope (for extraction-capable plugins). */
  storageScope?: string;
  schemaVersion?: number;
  publisher?: string;
  providers?: ProviderDescriptorV1[];
}

export interface PluginCatalogSource {
  id: string;
  name: string;
  url?: string;
  plugins: PluginInfo[];
  error?: string;
}

const CUSTOM_CATALOGS_KEY = "viewit-plugin-custom-catalogs";

export type ViewItBuildProfile = "development" | "device-test" | "production";

export function viewitBuildProfile(): ViewItBuildProfile {
  const profile = import.meta.env.VITE_VIEWIT_APP_PROFILE;
  return profile === "production" || profile === "device-test" ? profile : "development";
}

export function isInstallablePlugin(plugin: PluginInfo): boolean {
  return Boolean(plugin.downloadUrl && plugin.checksum && (plugin.sizeBytes ?? 0) > 0);
}

export function hasAndroidBridge(): boolean {
  return typeof window !== "undefined" && "AndroidBridge" in window;
}

export function formatPluginSize(bytes?: number): string {
  if (!bytes || bytes <= 0) return "size unknown";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export async function listInstalledPlugins(): Promise<PluginInfo[]> {
  if (!hasAndroidBridge()) return [];
  return JSON.parse((window as any).AndroidBridge.listPlugins()) as PluginInfo[];
}

export async function fetchPluginCatalog(): Promise<PluginInfo[]> {
  if (!hasAndroidBridge()) return [];
  return new Promise<PluginInfo[]>((resolve) => {
    const id = `cat_${Date.now()}_${Math.random().toString(36).slice(2)}`;
    (window as any)._catalogCallback = (cbId: string, data: string) => {
      if (cbId !== id) return;
      delete (window as any)._catalogCallback;
      resolve((JSON.parse(data) as PluginInfo[]).filter(isInstallablePlugin));
    };
    (window as any).AndroidBridge.fetchPluginCatalog(id);
  });
}

function normalizeCatalogPlugin(raw: any): PluginInfo | null {
  const formats = raw?.formats ?? raw?.supportedFormats;
  if (!raw?.id || !raw?.name || !raw?.version || !Array.isArray(formats)) return null;
  return {
    id: String(raw.id),
    name: String(raw.name),
    version: String(raw.version),
    description: String(raw.description ?? ""),
    formats: formats.map(String),
    downloadUrl: raw.downloadUrl ? String(raw.downloadUrl) : undefined,
    sizeBytes: Number(raw.sizeBytes ?? 0),
    installedSizeBytes: Number(raw.installedSizeBytes ?? 0),
    checksum: raw.checksum ? String(raw.checksum) : undefined,
    abi: raw.abi ? String(raw.abi) : undefined,
    abiVersion: Number(raw.abiVersion ?? 1),
    minAppVersion: Number(raw.minAppVersion ?? 1),
    entryClass: raw.entryClass ? String(raw.entryClass) : undefined,
    storageScope: raw.storageScope ? String(raw.storageScope) : undefined,
    base: raw.base ? String(raw.base) : "view",
    runtime: raw.runtime ? String(raw.runtime) : undefined,
    jsEntry: raw.jsEntry ? String(raw.jsEntry) : undefined,
    cssEntry: raw.cssEntry ? String(raw.cssEntry) : undefined,
    capabilities: Array.isArray(raw.capabilities) ? raw.capabilities.map(String) : undefined,
    schemaVersion: Number(raw.schemaVersion ?? 0),
    publisher: raw.publisher ? String(raw.publisher) : undefined,
    providers: Array.isArray(raw.providers) ? raw.providers : undefined,
  };
}

function normalizeCatalogBody(body: any): PluginInfo[] {
  const plugins = body?.catalog?.plugins ?? body?.plugins ?? [];
  if (!Array.isArray(plugins)) return [];
  return plugins
    .map(normalizeCatalogPlugin)
    .filter((plugin): plugin is PluginInfo => Boolean(plugin))
    .filter(isInstallablePlugin);
}

export function getCustomCatalogUrls(): string[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const parsed = JSON.parse(localStorage.getItem(CUSTOM_CATALOGS_KEY) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((url) => typeof url === "string") : [];
  } catch {
    return [];
  }
}

export function saveCustomCatalogUrls(urls: string[]): void {
  if (typeof localStorage === "undefined") return;
  const unique = Array.from(new Set(urls.map((url) => url.trim()).filter(Boolean)));
  localStorage.setItem(CUSTOM_CATALOGS_KEY, JSON.stringify(unique));
}

export async function fetchCustomCatalog(url: string): Promise<PluginInfo[]> {
  const bridge = hasAndroidBridge() ? (window as any).AndroidBridge : null;
  if (bridge && typeof bridge.fetchPluginCatalogFromUrl === "function") {
    return new Promise<PluginInfo[]>((resolve, reject) => {
      const id = `custom_cat_${Date.now()}_${Math.random().toString(36).slice(2)}`;
      const previousCallback = (window as any)._customCatalogCallback;
      (window as any)._customCatalogCallback = (payload: {
        id: string;
        plugins?: PluginInfo[];
        error?: string;
      }) => {
        if (payload.id !== id) {
          previousCallback?.(payload);
          return;
        }
        (window as any)._customCatalogCallback = previousCallback;
        if (payload.error) reject(new Error(payload.error));
        else resolve((payload.plugins ?? []).filter(isInstallablePlugin));
      };
      bridge.fetchPluginCatalogFromUrl(url, id);
    });
  }
  const response = await fetch(url, { cache: "no-store" });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  return normalizeCatalogBody(await response.json());
}

export async function fetchPluginCatalogSources(): Promise<PluginCatalogSource[]> {
  const defaultPlugins = (await fetchPluginCatalog()).map((plugin) => ({
    ...plugin,
    sourceId: "default",
    sourceName: "ViewIt default catalog",
  }));
  const sources: PluginCatalogSource[] = [
    { id: "default", name: "ViewIt default catalog", plugins: defaultPlugins },
  ];
  for (const url of getCustomCatalogUrls()) {
    try {
      const plugins = (await fetchCustomCatalog(url)).map((plugin) => ({
        ...plugin,
        sourceId: url,
        sourceName: url,
        sourceUrl: url,
      }));
      sources.push({ id: url, name: url, url, plugins });
    } catch (e) {
      sources.push({
        id: url,
        name: url,
        url,
        plugins: [],
        error: e instanceof Error ? e.message : String(e),
      });
    }
  }
  return sources;
}

export async function pluginInventory(): Promise<{
  installed: PluginInfo[];
  catalog: PluginInfo[];
}> {
  const installed = await listInstalledPlugins();
  const catalog = (await fetchPluginCatalogSources()).flatMap((source) => source.plugins);
  const installedById = new Map(installed.map((plugin) => [plugin.id, plugin]));
  const mergedByRuntime = new Map<string, PluginInfo>();
  for (const item of catalog) {
    const key = `${item.id}:${item.abi ?? ""}`;
    const previous = mergedByRuntime.get(key);
    mergedByRuntime.set(key, {
      ...previous,
      ...item,
      formats: Array.from(new Set([...(previous?.formats ?? []), ...item.formats])),
    });
  }
  const mergedCatalog = Array.from(mergedByRuntime.values()).map((item) => {
    const installedPlugin = installedById.get(item.id);
    const installedVersion = installedPlugin?.version;
    const versionMatches = installedVersion === item.version;
    return {
      ...item,
      installed: Boolean(installedPlugin && versionMatches),
      installedVersion,
      updateAvailable: Boolean(installedPlugin && !versionMatches),
    };
  });
  return { installed, catalog: mergedCatalog };
}

export function pluginSupports(plugin: PluginInfo, ext: string): boolean {
  return providerSupportsFormat(plugin, ext);
}

export async function resolveAvailableFormat(ext: string) {
  const inventory = await pluginInventory();
  return resolveFormatCapability(ext, inventory.installed, inventory.catalog);
}

export function materializeExternalUri(uri: string, ext: string): string {
  if (!hasAndroidBridge()) return uri;
  const bridge = (window as any).AndroidBridge;
  if (typeof bridge.materializeExternalUri !== "function") return uri;
  return bridge.materializeExternalUri(uri, ext);
}

export function isJsPlugin(plugin: PluginInfo): boolean {
  return plugin.runtime === "js";
}

const jsPluginModuleCache = new Map<string, Promise<Record<string, any>>>();

function decodeB64Utf8(b64: string): string {
  const raw = atob(b64);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return new TextDecoder("utf-8").decode(bytes);
}

function b64ToBytes(b64: string): Uint8Array {
  const raw = atob(b64);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return bytes;
}

/** Bundles are gzip+base64 across the bridge ("b64gz:") when they exceed ~1 MB. */
async function decodePluginCode(payload: string): Promise<string> {
  if (payload.startsWith("b64gz:")) {
    const gz = b64ToBytes(payload.slice(6));
    const gzBuf = gz.buffer.slice(gz.byteOffset, gz.byteOffset + gz.byteLength) as ArrayBuffer;
    const stream = new Blob([gzBuf]).stream().pipeThrough(new DecompressionStream("gzip"));
    return await new Response(stream).text();
  }
  return decodeB64Utf8(payload);
}

/**
 * Load a runtime=js plugin's bundle into the WebView and return its exports.
 * The bundle is read as base64 through the Android bridge (it lives in
 * app-private storage), gzip-decoded, and executed via indirect `eval` — the
 * WebView blocks `import()` of blob: URLs and never runs dynamically created
 * inline <script> nodes, so IIFE bundles exposed on `window.ViewItPlugin__<id>`
 * are the only reliable path (sync, no fetch, no CSP-sensitive DOM tricks).
 * Cached per plugin version.
 */
export async function loadJsPlugin(plugin: PluginInfo): Promise<Record<string, any>> {
  const key = `${plugin.id}@${plugin.version}`;
  const cached = jsPluginModuleCache.get(key);
  if (cached) return cached;

  if (!hasAndroidBridge()) {
    throw new Error(`JS plugin ${plugin.id} requires the Android app`);
  }
  const bridge = (window as any).AndroidBridge;
  const payload: Promise<Record<string, any>> = (async () => {
    const entryB64 =
      typeof bridge.loadPluginBundle === "function" ? bridge.loadPluginBundle(plugin.id) : "";
    if (!entryB64) throw new Error(`Could not read ${plugin.id} bundle (is it installed?)`);

    if (plugin.cssEntry && typeof bridge.pluginAssetB64 === "function") {
      const cssB64 = bridge.pluginAssetB64(plugin.id, plugin.cssEntry);
      if (cssB64) {
        try {
          let styleEl = document.getElementById(`viewit-plugin-css-${plugin.id}`);
          if (!styleEl) {
            styleEl = document.createElement("style");
            styleEl.id = `viewit-plugin-css-${plugin.id}`;
            document.head.appendChild(styleEl);
          }
          styleEl.textContent = decodeB64Utf8(cssB64);
        } catch {
          /* non-fatal */
        }
      }
    }

    const code = await decodePluginCode(entryB64);
    const globalName = `ViewItPlugin__${plugin.id.replace(/[^a-zA-Z0-9_]/g, "_")}`;
    if (typeof (window as any)[globalName] === "undefined") {
      (0, eval)(code);
    }
    const mod = (window as any)[globalName];
    if (!mod || typeof mod !== "object") {
      throw new Error(`${plugin.id} bundle did not expose ${globalName}`);
    }
    return mod as Record<string, any>;
  })();

  jsPluginModuleCache.set(key, payload);
  payload.catch(() => jsPluginModuleCache.delete(key));
  return payload;
}

/** True when an install/upgrade error means a native update is staged and needs a cold restart. */
export function isRestartToApplyError(err: unknown): boolean {
  return /restart the app to apply/i.test(err instanceof Error ? err.message : String(err));
}

/** Ask Android to cold-restart the app (applies a staged native-plugin update). */
export function restartApp(): void {
  if (!hasAndroidBridge()) return;
  (window as any).AndroidBridge.restartApp();
}

export function saveEditedText(
  text: string,
  displayName: string,
  mime = "text/plain",
): Promise<number> {
  if (!hasAndroidBridge()) {
    const blob = new Blob([text], { type: mime });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = displayName;
    a.click();
    URL.revokeObjectURL(a.href);
    return Promise.resolve(text.length);
  }
  return new Promise<number>((resolve, reject) => {
    const id = `edit_save_${Date.now()}_${Math.random().toString(36).slice(2)}`;
    const prev = (window as any)._editorSaveCallback;
    (window as any)._editorSaveCallback = (payload: {
      id: string;
      ok?: boolean;
      size?: number;
      error?: string;
    }) => {
      if (payload.id !== id) {
        prev?.(payload);
        return;
      }
      (window as any)._editorSaveCallback = prev;
      if (payload.ok) resolve(payload.size ?? text.length);
      else reject(new Error(payload.error || "Save failed"));
    };
    (window as any).AndroidBridge.saveEditedText(text, displayName, mime, id);
  });
}

export async function installPlugin(plugin: PluginInfo): Promise<void> {
  if (!hasAndroidBridge()) return;
  if (!isInstallablePlugin(plugin)) {
    throw new Error(`${plugin.name} is not available for download yet`);
  }
  const manifest = JSON.stringify(installManifest(plugin));
  return new Promise<void>((resolve, reject) => {
    const id = `install_${plugin.id}_${Date.now()}_${Math.random().toString(36).slice(2)}`;
    const bridge = window as any;
    const previousCallback = bridge._pluginCallback;
    bridge._pluginCallback = (payload: {
      id: string;
      event?: string;
      error?: string;
      progress?: number;
    }) => {
      if (payload.id !== id) {
        previousCallback?.(payload);
        return;
      }
      if (payload.event === "complete") {
        bridge._pluginCallback = previousCallback;
        resolve();
      } else if (payload.event === "error") {
        bridge._pluginCallback = previousCallback;
        reject(new Error(payload.error || "Install failed"));
      }
    };
    bridge.AndroidBridge.installPlugin(manifest, id);
  });
}

/** Preserve the complete verified catalog policy when crossing back into Android. */
export function installManifest(plugin: PluginInfo) {
  return {
    id: plugin.id,
    name: plugin.name,
    version: plugin.version,
    description: plugin.description,
    entryClass: plugin.entryClass ?? `ai.viewit.plugins.${plugin.id.replace(/-/g, "_")}.Plugin`,
    supportedFormats: plugin.formats,
    downloadUrl: plugin.downloadUrl,
    sizeBytes: plugin.sizeBytes ?? 0,
    installedSizeBytes: plugin.installedSizeBytes ?? 0,
    minAppVersion: plugin.minAppVersion ?? 1,
    checksum: plugin.checksum ?? "",
    abi: plugin.abi ?? "",
    abiVersion: plugin.abiVersion ?? 1,
    base: plugin.base ?? "view",
    capabilities: plugin.capabilities ?? [],
    schemaVersion: plugin.schemaVersion ?? 0,
    publisher: plugin.publisher ?? "",
    providers: plugin.providers ?? [],
    runtime: plugin.runtime ?? "native",
    jsEntry: plugin.runtime === "js" ? (plugin.jsEntry ?? "web/index.js") : "",
    cssEntry: plugin.runtime === "js" ? (plugin.cssEntry ?? "") : "",
  };
}

export async function removePlugin(pluginId: string): Promise<void> {
  if (!hasAndroidBridge()) return;
  (window as any).AndroidBridge.removePlugin(pluginId);
}

export interface ArchiveEntry {
  name: string;
  size: number;
  compressed_size: number;
  is_dir: boolean;
}

export interface ArchiveBridgeResult {
  ok: boolean;
  error?: string;
  tooLarge?: boolean;
  entries?: ArchiveEntry[];
  base64?: string;
  size?: number;
  count?: number;
  dir?: string;
  result?: Record<string, unknown>;
  format?: string;
}

export interface ArchiveBridgeApi {
  plugin: PluginInfo;
  uri: string;
  name: string;
  /** List the archive's contents (native plugin, non-destructive preview). */
  listArchive(): Promise<ArchiveBridgeResult>;
  /** Sniff the container format by magic bytes (works for misnamed archives, e.g. "archive.7z.enc"). */
  detectFormat(): Promise<string>;
  /** Read a single entry as bytes for in-place preview. Falls back to save-if-too-large. */
  readEntry(entryName: string): Promise<ArchiveBridgeResult>;
  /** Extract the entire archive to a user-chosen SAF folder (folder picker). */
  extractAll(): Promise<ArchiveBridgeResult>;
  /** Send one entry to the system storage-saver (ACTION_CREATE_DOCUMENT). */
  saveEntry(entryName: string, displayName?: string, mime?: string): Promise<ArchiveBridgeResult>;
}

export function archiveBridgeFor(
  plugin: PluginInfo,
  uri: string,
  name: string,
): ArchiveBridgeApi | null {
  if (!hasAndroidBridge()) return null;
  return {
    plugin,
    uri,
    name,
    async listArchive() {
      return callPluginArchive("list", plugin, uri, name);
    },
    async detectFormat() {
      const res = await callPluginArchive("detect", plugin, uri, name);
      return res.format ?? "unknown";
    },
    async readEntry(entryName) {
      const res = await callPluginArchive("entry", plugin, uri, name, { entryName });
      if (!res.ok && !res.tooLarge) return res;
      return res;
    },
    async extractAll() {
      return callPluginArchive("extract-all", plugin, uri, name);
    },
    async saveEntry(entryName, displayName, mime) {
      return callPluginArchive("save", plugin, uri, name, {
        entryName,
        displayName: displayName || entryName.split("/").pop() || entryName,
        mime: mime || "",
      });
    },
  };
}

function callPluginArchive(
  op: string,
  plugin: PluginInfo,
  uri: string,
  name: string,
  extra: Record<string, string> = {},
): Promise<ArchiveBridgeResult> {
  return new Promise<ArchiveBridgeResult>((resolve) => {
    if (!hasAndroidBridge()) {
      resolve({ ok: false, error: "Archive preview is only available in the Android app" });
      return;
    }
    const id = `arch_${op}_${plugin.id}_${Date.now()}_${Math.random().toString(36).slice(2)}`;
    const previousCallback = (window as any)._pluginArchiveCallback;
    (window as any)._pluginArchiveCallback = (payload: { id: string }) => {
      if (payload.id !== id) {
        previousCallback?.(payload);
        return;
      }
      (window as any)._pluginArchiveCallback = previousCallback;
      const data = payload as Record<string, any>;
      const result: ArchiveBridgeResult = {
        ok: !data.error,
        error: data.error,
        tooLarge: data.tooLarge,
        base64: data.base64,
        size: data.size,
        dir: data.dir,
        result: data.result,
      };
      if (op === "detect") {
        result.format = data.format;
      }
      if (op === "list" && data.manifest) {
        result.entries = data.manifest.entries;
        result.format = data.manifest.format;
      }
      resolve(result);
    };
    const bridge = (window as any).AndroidBridge;
    switch (op) {
      case "list":
        bridge.listPluginArchiveAsync(plugin.id, uri, name, id);
        break;
      case "detect":
        bridge.detectPluginArchiveFormatAsync(plugin.id, uri, name, id);
        break;
      case "entry":
        bridge.extractPluginArchiveEntryAsync(plugin.id, uri, name, extra.entryName, id);
        break;
      case "extract-all":
        bridge.extractPluginArchiveAllToFolderAsync(plugin.id, uri, name, id);
        break;
      case "save":
        bridge.savePluginArchiveEntryAsync(
          plugin.id,
          uri,
          name,
          extra.entryName,
          extra.displayName,
          extra.mime,
          id,
        );
        break;
      default:
        resolve({ ok: false, error: `Unknown archive op ${op}` });
    }
  });
}

export async function renderDocumentWithPlugin(
  plugin: PluginInfo,
  uri: string,
  ext: string,
): Promise<unknown> {
  if (!hasAndroidBridge())
    throw new Error("Android document plugins are only available in the Android app");
  return new Promise((resolve, reject) => {
    const id = `doc_${Date.now()}_${Math.random().toString(36).slice(2)}`;
    (window as any)._documentPluginCallback = (payload: {
      id: string;
      document?: unknown;
      error?: string;
    }) => {
      if (payload.id !== id) return;
      delete (window as any)._documentPluginCallback;
      if (payload.error) reject(new Error(payload.error));
      else resolve(payload.document);
    };
    (window as any).AndroidBridge.renderDocumentWithPlugin(plugin.id, uri, ext, id);
  });
}
