<script lang="ts">
  import { untrack } from "svelte";
  import {
    androidBridgeAvailable,
    androidListDir,
    browseDir,
    hasAllFilesAccess,
    pickSingleFile,
    requestAllFilesAccess,
    type BrowseEntry,
    type BrowseListing,
  } from "@viewit/platform";
  import Icon from "./Icon.svelte";

  let { onPick, root = "desktop" }: { onPick: (file: File) => void; root?: string } = $props();

  // Web keeps the directory-input fallback; Tauri roots get real browsing.
  let webEntries: Array<{ name: string; size: number; url: string; isImage: boolean }> = $state([]);

  let listing: BrowseListing | null = $state(null);
  let browseError = $state("");
  let loading = $state(false);
  let permDenied = $state(false);

  function isAndroidBridge(): boolean {
    return root === "mobile" && androidBridgeAvailable();
  }

  function basename(p: string) {
    const i = p.lastIndexOf("/");
    return i >= 0 ? p.slice(i + 1) : p;
  }
  function extLabel(name: string) {
    const i = name.lastIndexOf(".");
    return i >= 0 ? name.slice(i + 1).toUpperCase() : "?";
  }
  function sizeLabel(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }

  async function load(path = "") {
    loading = true;
    browseError = "";
    try {
      if (isAndroidBridge()) {
        if (!hasAllFilesAccess()) {
          permDenied = true;
          listing = null;
          return;
        }
        permDenied = false;
        listing = androidListDir(path);
        if (!listing) {
          permDenied = !hasAllFilesAccess();
          browseError = "Could not read this folder.";
        }
      } else {
        const result = await browseDir(path);
        if (!result) {
          browseError = "Browsing is unavailable on this platform — use Open file instead.";
          return;
        }
        listing = result;
      }
    } finally {
      loading = false;
    }
  }

  // Bootstrap once. `load()` writes several $state signals synchronously (the
  // Android bridge path is fully sync), so it must run untracked — otherwise
  // the write→read cycle inside this effect can wedge Svelte's flush loop
  // (effect_update_depth_exceeded) and freeze all further UI updates.
  let bootstrapped = false;
  $effect(() => {
    if (bootstrapped) return;
    bootstrapped = true;
    const isAndroid = untrack(() => root === "mobile" && androidBridgeAvailable());
    if (root !== "web") void untrack(() => load(""));
    if (!isAndroid) return;
    // Re-check the Android grant when the user returns from Settings.
    const onVisible = () => {
      if (document.visibilityState === "visible") void load(listing?.path ?? "");
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  });

  function crumbs(): Array<{ label: string; path: string }> {
    if (!listing?.path) return [];
    const parts = listing.path.split("/").filter(Boolean);
    const out: Array<{ label: string; path: string }> = [];
    let acc = "";
    for (const part of parts) {
      acc += `/${part}`;
      out.push({ label: part, path: acc });
    }
    return out;
  }

  function openEntry(entry: BrowseEntry) {
    if (entry.isDir) {
      void load(entry.path);
      return;
    }
    // Absolute paths ride the existing open pipeline as file:// URIs
    // (streamed/materialized natively — never read fully into JS).
    const f = new File([], entry.name, { type: "application/octet-stream" });
    Object.defineProperty(f, "viewitUri", { value: `file://${entry.path}`, enumerable: true });
    onPick(f);
  }

  async function choose() {
    if (root === "mobile" || root === "desktop") {
      const f = await pickSingleFile();
      if (f) onPick(f);
      return;
    }
    const input = document.createElement("input");
    input.type = "file";
    (input as HTMLInputElement & { webkitdirectory?: boolean }).webkitdirectory = true;
    input.multiple = true;
    input.onchange = () => {
      const files = Array.from(input.files ?? []);
      webEntries = files.slice(0, 500).map((f) => {
        const type = f.type || "";
        const isImage = type.startsWith("image/");
        return {
          name: (f as File & { webkitRelativePath?: string }).webkitRelativePath || f.name,
          size: f.size,
          url: isImage ? URL.createObjectURL(f) : "",
          isImage,
        };
      });
    };
    input.click();
  }
</script>

<div class="grid-view">
  <div class="browse-bar">
    <button type="button" onclick={choose}>Choose file…</button>
    {#if listing}
      <nav class="crumbs" aria-label="Folder path">
        {#each crumbs() as crumb, i}
          {#if i > 0}<span class="sep">/</span>{/if}
          <button
            type="button"
            class="crumb"
            class:current={i === crumbs().length - 1}
            onclick={() => void load(crumb.path)}>{crumb.label}</button
          >
        {/each}
      </nav>
    {/if}
  </div>

  {#if root === "web"}
    {#if webEntries.length > 0}
      <div class="grid" role="grid">
        {#each webEntries as entry}
          <div class="cell static" role="gridcell">
            {#if entry.isImage}
              <img src={entry.url} alt={entry.name} loading="lazy" />
            {:else}
              <div class="icon">{extLabel(entry.name)}</div>
            {/if}
            <span class="label">{basename(entry.name)}</span>
          </div>
        {/each}
      </div>
    {/if}
  {:else if permDenied}
    <div class="perm-card" role="alert">
      <strong>Browse needs all-files access</strong>
      <p>
        Android only lets ViewIt list your folders after you allow
        <em>All files access</em> for the app. The system picker ("Open file") works without it.
      </p>
      <button type="button" onclick={() => requestAllFilesAccess()}>
        Allow all files access…
      </button>
      <p class="perm-hint">Return here after granting — the folder loads automatically.</p>
    </div>
  {:else if loading && !listing}
    <p class="status">Loading folder…</p>
  {:else if browseError}
    <p class="status error">{browseError}</p>
  {:else if listing}
    <div class="grid" role="grid">
      {#if listing.parent !== "" && listing.parent !== listing.path}
        <button type="button" class="cell dir-up" onclick={() => void load(listing!.parent)}>
          <div class="icon dir"><Icon name="arrow-left" /></div>
          <span class="label">Up one level</span>
        </button>
      {/if}
      {#each listing.entries as entry (entry.path)}
        <button type="button" class="cell" onclick={() => openEntry(entry)} role="gridcell">
          {#if entry.isDir}
            <div class="icon dir"><Icon name="folder" /></div>
          {:else}
            <div class="icon">{extLabel(entry.name)}</div>
          {/if}
          <span class="label">{entry.name}</span>
          {#if !entry.isDir}<span class="size">{sizeLabel(entry.size)}</span>{/if}
        </button>
      {/each}
    </div>
    {#if listing.entries.length === 0}
      <p class="status">This folder is empty.</p>
    {/if}
  {/if}
</div>

<style>
  .grid-view {
    padding: 1rem;
  }
  .browse-bar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
    margin-bottom: 1rem;
  }
  .browse-bar > button {
    cursor: pointer;
    background: var(--bg-secondary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    padding: 0.4rem 0.8rem;
  }
  .crumbs {
    display: flex;
    align-items: center;
    gap: 0.15rem;
    flex-wrap: wrap;
    min-width: 0;
    font-size: 0.8rem;
  }
  .crumbs .sep {
    color: var(--text-secondary);
  }
  .crumb {
    border: 0;
    background: none;
    color: var(--link);
    cursor: pointer;
    padding: 0.2rem 0.25rem;
    border-radius: 0.25rem;
    max-width: 11rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .crumb.current {
    color: var(--text-primary);
    font-weight: 600;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 1rem;
  }
  .cell {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.5rem;
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    cursor: pointer;
    text-align: center;
    min-width: 44px;
    min-height: 44px;
  }
  .cell:hover {
    background: var(--bg-secondary);
  }
  .cell.static {
    cursor: default;
  }
  .dir-up {
    opacity: 0.85;
  }
  .cell img {
    width: 100%;
    height: 96px;
    object-fit: cover;
    border-radius: 0.3rem;
  }
  .icon {
    height: 96px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-secondary);
    border-radius: 0.3rem;
    font-family: ui-monospace, monospace;
    font-weight: 600;
    color: var(--text-secondary);
  }
  .icon.dir {
    color: var(--link);
  }
  .label {
    font-size: 0.75rem;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .size {
    font-size: 0.65rem;
    color: var(--text-secondary);
  }
  .status {
    color: var(--text-secondary);
    font-style: italic;
  }
  .status.error {
    color: var(--error);
    font-style: normal;
  }
  .perm-card {
    max-width: 30rem;
    margin: 2rem auto 0;
    padding: 1rem 1.1rem;
    border: 1px solid var(--border);
    border-radius: 0.8rem;
    background: var(--bg-secondary);
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .perm-card p {
    margin: 0;
    color: var(--text-secondary);
    font-size: 0.85rem;
  }
  .perm-card button {
    align-self: flex-start;
    min-height: 44px;
    padding: 0.45rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 0.45rem;
    background: var(--bg-primary);
    color: var(--text-primary);
    cursor: pointer;
  }
  .perm-hint {
    font-size: 0.75rem;
  }
</style>
