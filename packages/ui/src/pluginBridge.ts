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
  entryClass?: string;
  installed?: boolean;
  installedVersion?: string;
  updateAvailable?: boolean;
}

export function hasAndroidBridge(): boolean {
  return typeof window !== 'undefined' && 'AndroidBridge' in window;
}

export function formatPluginSize(bytes?: number): string {
  if (!bytes || bytes <= 0) return 'size unknown';
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
      resolve(JSON.parse(data) as PluginInfo[]);
    };
    (window as any).AndroidBridge.fetchPluginCatalog(id);
  });
}

export async function pluginInventory(): Promise<{ installed: PluginInfo[]; catalog: PluginInfo[] }> {
  const installed = await listInstalledPlugins();
  const catalog = await fetchPluginCatalog();
  const installedById = new Map(installed.map((plugin) => [plugin.id, plugin]));
  const seen = new Set<string>();
  const mergedCatalog = catalog
    .filter((item) => {
      const key = `${item.id}:${item.abi ?? ''}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    })
    .map((item) => {
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
  const cleanExt = ext.toLowerCase().replace(/^\./, '');
  return plugin.formats?.some((format) => format.toLowerCase() === cleanExt) ?? false;
}

export function materializeExternalUri(uri: string, ext: string): string {
  if (!hasAndroidBridge()) return uri;
  const bridge = (window as any).AndroidBridge;
  if (typeof bridge.materializeExternalUri !== 'function') return uri;
  return bridge.materializeExternalUri(uri, ext);
}

export async function installPlugin(plugin: PluginInfo): Promise<void> {
  if (!hasAndroidBridge()) return;
  const manifest = JSON.stringify({
    id: plugin.id,
    name: plugin.name,
    version: plugin.version,
    description: plugin.description,
    entryClass: plugin.entryClass ?? `ai.viewit.plugins.${plugin.id.replace(/-/g, '_')}.Plugin`,
    supportedFormats: plugin.formats,
    downloadUrl: plugin.downloadUrl,
    sizeBytes: plugin.sizeBytes ?? 0,
    installedSizeBytes: plugin.installedSizeBytes ?? 0,
    minAppVersion: 1,
    checksum: plugin.checksum ?? '',
    abi: plugin.abi ?? '',
  });
  (window as any).AndroidBridge.installPlugin(manifest, plugin.id);
}

export async function removePlugin(pluginId: string): Promise<void> {
  if (!hasAndroidBridge()) return;
  (window as any).AndroidBridge.removePlugin(pluginId);
}

export async function renderDocumentWithPlugin(plugin: PluginInfo, uri: string, ext: string): Promise<unknown> {
  if (!hasAndroidBridge()) throw new Error('Android document plugins are only available in the Android app');
  return new Promise((resolve, reject) => {
    const id = `doc_${Date.now()}_${Math.random().toString(36).slice(2)}`;
    (window as any)._documentPluginCallback = (payload: { id: string; document?: unknown; error?: string }) => {
      if (payload.id !== id) return;
      delete (window as any)._documentPluginCallback;
      if (payload.error) reject(new Error(payload.error));
      else resolve(payload.document);
    };
    (window as any).AndroidBridge.renderDocumentWithPlugin(plugin.id, uri, ext, id);
  });
}
