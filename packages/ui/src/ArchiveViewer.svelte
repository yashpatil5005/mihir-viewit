<script lang="ts">
  import Icon from "./Icon.svelte";
  // Archive viewer with drill-down.
  // Tap on a file entry → open it in-place (native plugin: single-member
  // preview without extracting the whole archive; Tauri/web: wasm extract).
  // When the compression-universal plugin is installed the toolbar offers
  // per-entry Save and full "Extract all" into a user-chosen folder.
  // Storage scope note: plugin writes go to the app-specific external
  // storage directory — no runtime permission is required on any API level.
  let {
    entries = [],
    format = "archive-zip",
    byte_len = 0,
    name = "",
    uri = "",
    renderer = undefined,
  }: {
    entries: { name: string; size: number; is_dir: boolean; compressed_size: number }[];
    format: string;
    byte_len: number;
    name?: string;
    uri?: string;
    renderer?: { id?: string; label?: string };
  } = $props();

  import { onMount } from "svelte";
  import { openFile } from "@viewit/platform";
  import {
    archiveBridgeFor,
    hasAndroidBridge,
    isJsPlugin,
    listInstalledPlugins,
    pluginSupports,
    type ArchiveBridgeApi,
    type PluginInfo,
  } from "./pluginBridge";

  let plugin: PluginInfo | null = $state(null);
  let bridge: ArchiveBridgeApi | null = $state(null);
  let busy = $state("");
  let notice = $state("");
  let error = $state("");
  let extractDir: string | null = $state(null);
  let currentPath = $state("");

  type ArchiveEntry = (typeof entries)[number];
  type VisibleEntry = ArchiveEntry & { displayName: string; path: string; synthetic?: boolean };

  let breadcrumbs = $derived(
    currentPath
      .split("/")
      .filter(Boolean)
      .map((label, index, parts) => ({ label, path: `${parts.slice(0, index + 1).join("/")}/` })),
  );
  let visibleEntries = $derived.by(() => {
    const children = new Map<string, VisibleEntry>();
    for (const entry of entries) {
      const normalized = entry.name.replace(/^\/+/, "");
      if (!normalized.startsWith(currentPath) || normalized === currentPath) continue;
      const relative = normalized.slice(currentPath.length);
      const [first, ...rest] = relative.split("/");
      if (!first) continue;
      const path = `${currentPath}${first}${rest.length > 0 || entry.is_dir ? "/" : ""}`;
      if (rest.length > 0) {
        if (!children.has(first)) {
          children.set(first, {
            name: path,
            path,
            displayName: first,
            size: 0,
            compressed_size: 0,
            is_dir: true,
            synthetic: true,
          });
        }
      } else {
        children.set(first, { ...entry, path, displayName: first.replace(/\/$/, "") });
      }
    }
    return [...children.values()].sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      return a.displayName.localeCompare(b.displayName, undefined, { numeric: true });
    });
  });

  function openFolder(path: string) {
    currentPath = path.endsWith("/") ? path : `${path}/`;
  }

  function goToBreadcrumb(index: number) {
    currentPath = index < 0 ? "" : breadcrumbs[index].path;
  }

  const TOO_LARGE = "Too large for in-place preview — use Save to extract this member.";

  function decodeB64(b64: string): Uint8Array {
    const raw = atob(b64);
    const bytes = new Uint8Array(raw.length);
    for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
    return bytes;
  }

  async function resolvePlugin(): Promise<void> {
    if (!hasAndroidBridge()) return;
    try {
      const ext = extFromFormat(format);
      const installed = await listInstalledPlugins();
      const byRenderer = renderer?.id ? installed.find((p) => p.id === renderer.id) : undefined;
      const byFormat = installed.find(
        (p) =>
          p.id === "compression-universal" &&
          !isJsPlugin(p) &&
          (pluginSupports(p, ext) || pluginSupports(p, "zip")),
      );
      const resolved = byRenderer ?? byFormat ?? null;
      if (resolved) {
        plugin = resolved;
        bridge = archiveBridgeFor(resolved, uri, name);
        (window as any).__viewitArchiveBridge = { pluginId: resolved.id, uri, name };
      }
    } catch (e) {
      console.error("resolve compression plugin failed:", e);
    }
  }

  function extFromFormat(fmt: string): string {
    const map: Record<string, string> = {
      "archive-zip": "zip",
      "archive-tar": "tar",
      "archive-tar-gz": "gz",
      "archive-7z": "7z",
      "archive-rar": "rar",
    };
    return map[fmt] ?? fmt.split(".").pop() ?? fmt;
  }

  async function openMember(entry: any) {
    if (entry.is_dir) return;
    busy = `Opening ${entry.name}…`;
    error = "";
    notice = "";
    try {
      if (bridge) {
        const res = await bridge.readEntry(entry.name);
        if (res.error) {
          error = res.error;
          if (res.tooLarge) notice = TOO_LARGE;
          return;
        }
        const bytes = decodeB64(res.base64 ?? "");
        const blob = new Blob([bytes.buffer as ArrayBuffer], { type: "application/octet-stream" });
        const url = URL.createObjectURL(blob);
        await openFile(url, entry.name.split("/").pop() ?? entry.name);
        URL.revokeObjectURL(url);
        return;
      }
      if (typeof (window as any).__TAURI_INTERNALS__ === "undefined") {
        error = `In-place preview of ${entry.name} needs the Compression Universal plugin (Android) or the desktop app.`;
        return;
      }
      const invoke = (await import("@tauri-apps/api/core")).invoke;
      const bytes: number[] = await invoke("archive_extract", {
        uri,
        entryName: entry.name,
      });
      const blob = new Blob([new Uint8Array(bytes)], { type: "application/octet-stream" });
      const url = URL.createObjectURL(blob);
      await openFile(url);
      URL.revokeObjectURL(url);
    } catch (e) {
      console.error("archive member open failed:", e);
      error = `Failed to open ${entry.name}: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      busy = "";
    }
  }

  async function saveMember(entry: any) {
    if (!bridge) return;
    busy = `Saving ${entry.name}…`;
    error = "";
    notice = "";
    try {
      const res = await bridge.saveEntry(entry.name);
      if (res.error) error = res.error;
      else notice = `Saved ${entry.displayName ?? entry.name.split("/").pop() ?? entry.name}`;
    } finally {
      busy = "";
    }
  }

  async function extractAll() {
    if (!bridge) return;
    busy = "Choose a destination folder…";
    error = "";
    notice = "";
    try {
      const res = await bridge.extractAll();
      if (res.error) {
        error = res.error;
      } else {
        extractDir = res.dir ?? null;
        const count = res.count ?? res.result?.files_count ?? res.result?.count ?? 0;
        notice = `Extracted ${count} file${count === 1 ? "" : "s"} to ${res.dir ?? "the chosen folder"}`;
      }
    } finally {
      busy = "";
    }
  }

  onMount(() => {
    (window as any).__viewitArchiveSource = { uri, name };
    resolvePlugin();
    return () => {
      delete (window as any).__viewitArchiveSource;
      delete (window as any).__viewitArchiveBridge;
    };
  });
</script>

<article class="archive-viewer">
  <aside class="meta">
    <strong>{entries.length} entries</strong> · {format}
    {#if byte_len > 0}
      · {byte_len.toLocaleString()} bytes{/if}
    {#if renderer?.label}
      · listed by <em>{renderer.label}</em>{/if}
  </aside>

  {#if plugin}
    <div class="toolbar">
      <button class="act" onclick={extractAll} disabled={Boolean(busy)}>Export all</button>
      <span class="scope">Choose a folder to extract into — no storage permission needed.</span>
    </div>
  {/if}

  {#if busy}<p class="busy">{busy}</p>{/if}
  {#if notice}<p class="notice">{notice}</p>{/if}
  {#if error}<p class="error">{error}</p>{/if}
  {#if extractDir}
    <p class="scope">
      Exported to <code>{extractDir}</code>. ViewIt extracted directly into the folder you picked
      using the system document picker.
    </p>
  {/if}

  <nav class="breadcrumbs" aria-label="Archive path">
    <button class:current={!currentPath} onclick={() => goToBreadcrumb(-1)}>Archive</button>
    {#each breadcrumbs as crumb, index}
      <span aria-hidden="true">/</span>
      <button
        class:current={index === breadcrumbs.length - 1}
        onclick={() => goToBreadcrumb(index)}
      >
        {crumb.label}
      </button>
    {/each}
  </nav>

  <div class="table-frame">
    <table>
      <thead>
        <tr
          ><th>Name</th><th>Size</th><th>Compressed</th>{#if plugin}<th></th>{/if}</tr
        >
      </thead>
      <tbody>
        {#if currentPath}
          <tr class="dir">
            <td>
              <button class="drill folder" onclick={() => goToBreadcrumb(breadcrumbs.length - 2)}>
                <Icon name="folder-open" size={18} /> <span>..</span>
              </button>
            </td>
            <td></td><td></td>{#if plugin}<td></td>{/if}
          </tr>
        {/if}
        {#each visibleEntries as entry}
          <tr class:dir={entry.is_dir}>
            <td>
              {#if entry.is_dir}
                <button class="drill folder" onclick={() => openFolder(entry.path)}>
                  <Icon name="folder" size={18} /> <span>{entry.displayName}</span>
                </button>
              {:else}
                <button
                  class="drill"
                  onclick={() => openMember(entry)}
                  aria-label="Open {entry.name}"
                >
                  <Icon name="file" size={18} /> <span>{entry.displayName}</span>
                </button>
              {/if}
            </td>
            <td>{entry.size.toLocaleString()}</td>
            <td>{entry.compressed_size.toLocaleString()}</td>
            {#if plugin}
              <td>
                {#if !entry.is_dir}
                  <button
                    class="save"
                    onclick={() => saveMember(entry)}
                    aria-label="Save {entry.name}">Save…</button
                  >
                {/if}
              </td>
            {/if}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</article>

<style>
  .archive-viewer {
    padding: 0.5rem 1rem;
    color: var(--text-primary);
  }
  .drill {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    min-height: 44px;
  }
  .meta {
    color: var(--text-secondary);
    margin-bottom: 0.5rem;
    font-size: 0.75rem;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin: 0 0 0.5rem;
    flex-wrap: wrap;
  }
  .act {
    cursor: pointer;
    background: var(--bg-secondary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.35rem 0.8rem;
    font-size: 0.85rem;
  }
  .act:hover {
    background: var(--border);
  }
  .busy {
    color: var(--text-secondary);
    font-style: italic;
    margin: 0 0 0.5rem;
    font-size: 0.85rem;
  }
  .notice {
    color: var(--link);
    margin: 0 0 0.5rem;
    font-size: 0.85rem;
  }
  .error {
    color: var(--error);
    margin: 0 0 0.5rem;
    font-size: 0.85rem;
    white-space: pre-wrap;
  }
  .scope {
    color: var(--text-secondary);
    font-size: 0.78rem;
    margin: 0 0 0.5rem;
  }
  .scope code {
    background: var(--bg-secondary);
    padding: 0.05rem 0.3rem;
    border-radius: 0.25rem;
  }
  .table-frame {
    overflow: auto;
    max-height: 75vh;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
  }
  .breadcrumbs {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    overflow-x: auto;
    margin: 0 0 0.5rem;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--bg-secondary);
    white-space: nowrap;
  }
  .breadcrumbs button {
    border: 0;
    padding: 0.1rem 0.2rem;
    color: var(--link);
    background: transparent;
    cursor: pointer;
  }
  .breadcrumbs button.current {
    color: var(--text-primary);
    font-weight: 600;
    cursor: default;
  }
  table {
    border-collapse: collapse;
    width: 100%;
    font-family: ui-monospace, monospace;
    font-size: 0.85rem;
  }
  th,
  td {
    border-bottom: 1px solid var(--border);
    padding: 0.3rem 0.6rem;
    text-align: left;
  }
  th {
    background: var(--bg-secondary);
    font-weight: 600;
    position: sticky;
    top: 0;
  }
  .dir td {
    font-weight: 600;
    color: var(--text-primary);
  }
  .drill {
    cursor: pointer;
    background: none;
    border: none;
    padding: 0;
    color: var(--link);
    font-family: inherit;
    font-size: inherit;
    text-align: left;
  }
  .drill:hover {
    text-decoration: underline;
  }
  .drill.folder {
    color: var(--text-primary);
    font-weight: 600;
  }
  .save {
    cursor: pointer;
    background: none;
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.1rem 0.5rem;
    color: var(--text-secondary);
    font-size: 0.72rem;
  }
  .save:hover {
    color: var(--text-primary);
    background: var(--bg-secondary);
  }
</style>
