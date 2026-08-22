<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import {
    androidBridgeAvailable,
    androidListDir,
    browseDir,
    fileSrcUrl,
    friendlyCrumbs,
    hasAllFilesAccess,
    pickSingleFile,
    requestAllFilesAccess,
    warmFileSrc,
    type BrowseEntry,
    type BrowseListing,
    type Crumb,
  } from "@viewit/platform";
  import Icon from "./Icon.svelte";

  let {
    onPick,
    root = "desktop",
    initialPath = "",
    onListed = undefined,
    upSignal = 0,
  }: {
    onPick: (file: File) => void;
    root?: string;
    /** Folder to open on mount (back-navigation restore). */
    initialPath?: string;
    /** Fired after each successful listing with [path, parent|null]. */
    onListed?: (path: string, parent: string | null) => void;
    /** Increment to make the grid go up one folder. */
    upSignal?: number;
  } = $props();

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
    return i >= 0
      ? name
          .slice(i + 1)
          .toUpperCase()
          .slice(0, 4)
      : "FILE";
  }

  type FileType = "image" | "music" | "video" | "archive" | "file-text" | "file";
  const EXT_TYPES: Record<string, FileType> = {};
  for (const e of [
    "png",
    "jpg",
    "jpeg",
    "gif",
    "webp",
    "bmp",
    "svg",
    "heic",
    "heif",
    "avif",
    "tif",
    "tiff",
    "ico",
  ])
    EXT_TYPES[e] = "image";
  for (const e of [
    "mp3",
    "wav",
    "flac",
    "ogg",
    "oga",
    "m4a",
    "aac",
    "opus",
    "aiff",
    "aif",
    "wma",
    "mid",
    "midi",
  ])
    EXT_TYPES[e] = "music";
  for (const e of ["mp4", "mkv", "mov", "avi", "webm", "m4v", "wmv", "flv", "ts", "3gp"])
    EXT_TYPES[e] = "video";
  for (const e of [
    "zip",
    "7z",
    "rar",
    "tar",
    "gz",
    "bz2",
    "xz",
    "zst",
    "lz4",
    "lzma",
    "cab",
    "iso",
    "jar",
    "apk",
  ])
    EXT_TYPES[e] = "archive";
  function fileTypeIcon(name: string): FileType {
    const ext = (
      name.lastIndexOf(".") >= 0 ? name.slice(name.lastIndexOf(".") + 1) : name
    ).toLowerCase();
    return EXT_TYPES[ext] ?? "file-text";
  }
  // ---- Previews -----------------------------------------------------------
  // Images render as <img>, videos as first-frame <video> posters, both fed
  // by the asset protocol. Media mounts only once its cell scrolls into
  // view; failures fall back to the type icon.
  let srcVersion = $state(0);
  let visiblePaths = $state<Set<string>>(new Set());
  let failedPreviews = $state<Set<string>>(new Set());
  let observer: IntersectionObserver | null = null;
  const MAX_CONCURRENT_PREVIEWS = 6;
  let activePreviewCount = 0;
  let previewQueue: Array<{ path: string; node: HTMLElement }> = [];

  void (async () => {
    await warmFileSrc();
    srcVersion++;
  })();

  function markPreviewFailed(path: string) {
    const next = new Set(failedPreviews);
    next.add(path);
    failedPreviews = next;
  }

  function lazyPreview(node: HTMLElement, path: string) {
    observer?.observe(node);
    return {
      destroy() {
        observer?.unobserve(node);
        previewQueue = previewQueue.filter((item) => item.node !== node);
        if (visiblePaths.has(path)) {
          const next = new Set(visiblePaths);
          next.delete(path);
          visiblePaths = next;
          activePreviewCount = Math.max(0, activePreviewCount - 1);
        }
      },
    };
  }

  function sizeLabel(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }

  const PAGE_SIZE = 300;

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
        listing = androidListDir(path, 0, PAGE_SIZE);
        if (!listing) {
          permDenied = !hasAllFilesAccess();
          browseError = "Could not read this folder.";
        }
      } else {
        const result = await browseDir(path, 0, PAGE_SIZE);
        if (!result) {
          browseError = "Browsing is unavailable on this platform — use Open file instead.";
          return;
        }
        listing = result;
      }
    } finally {
      loading = false;
      if (listing) untrack(() => onListed?.(listing!.path, listing!.parent || null));
    }
  }

  // Append the next page when the sentinel scrolls into view. Entries are
  // accumulated into the same listing so the grid grows in place.
  let loadingMore = $state(false);
  async function loadMore() {
    if (loadingMore || !listing?.hasMore || loading) return;
    loadingMore = true;
    try {
      const { path, entries: current } = listing;
      const offset = current.length;
      const page = isAndroidBridge()
        ? androidListDir(path, offset, PAGE_SIZE)
        : await browseDir(path, offset, PAGE_SIZE);
      if (
        !page ||
        page.path !== path ||
        !page.entries.length ||
        !listing ||
        listing.path !== path
      ) {
        if (listing && listing.path === path && page) listing = { ...page, entries: [...current] };
        return;
      }
      // De-dup by path just in case the folder changed mid-scroll.
      const seen = new Set(current.map((e) => e.path));
      const fresh = page.entries.filter((e) => !seen.has(e.path));
      listing = {
        ...page,
        entries: [...current, ...fresh],
        hasMore: page.hasMore,
        total: page.total,
      };
    } finally {
      loadingMore = false;
    }
  }

  let moreSentinel: HTMLElement | null = $state(null);
  let sentinelObserver: IntersectionObserver | null = null;
  $effect(() => {
    if (!moreSentinel || !listing?.hasMore) return;
    sentinelObserver?.disconnect();
    sentinelObserver = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) void loadMore();
      },
      { rootMargin: "600px" },
    );
    sentinelObserver.observe(moreSentinel);
    return () => {
      sentinelObserver?.disconnect();
      sentinelObserver = null;
    };
  });

  // Parent-driven "go up one folder" (back navigation from the app shell).
  let lastUpSignal: number | null = null;
  $effect(() => {
    if (lastUpSignal === null) {
      lastUpSignal = upSignal;
      return;
    }
    if (upSignal !== lastUpSignal) {
      lastUpSignal = upSignal;
      const parent = untrack(() => listing?.parent ?? "");
      if (parent && parent !== listing?.path) void untrack(() => load(parent));
    }
  });

  function drainPreviewQueue() {
    while (activePreviewCount < MAX_CONCURRENT_PREVIEWS && previewQueue.length > 0) {
      const item = previewQueue.shift();
      if (!item) break;
      activePreviewCount++;
      const next = new Set(visiblePaths);
      next.add(item.path);
      visiblePaths = next;
    }
  }

  observer =
    typeof IntersectionObserver === "function"
      ? new IntersectionObserver(
          (entries) => {
            const entering: Array<{ path: string; node: HTMLElement }> = [];
            const leaving: string[] = [];
            for (const e of entries) {
              const path = (e.target as HTMLElement).dataset.previewPath;
              if (!path) continue;
              if (e.isIntersecting) {
                entering.push({ path, node: e.target as HTMLElement });
              } else {
                leaving.push(path);
              }
            }
            if (leaving.length > 0) {
              const next = new Set(visiblePaths);
              for (const p of leaving) next.delete(p);
              visiblePaths = next;
              activePreviewCount = Math.max(0, activePreviewCount - leaving.length);
            }
            if (entering.length > 0) {
              previewQueue.push(...entering);
              drainPreviewQueue();
            }
          },
          { root: null, rootMargin: "128px" },
        )
      : null;
  onDestroy(() => {
    observer?.disconnect();
    observer = null;
  });

  // Bootstrap once. `load()` writes several $state signals synchronously (the
  // Android bridge path is fully sync), so it must run untracked — otherwise
  // the write→read cycle inside this effect can wedge Svelte's flush loop
  // (effect_update_depth_exceeded) and freeze all further UI updates.
  let bootstrapped = false;
  $effect(() => {
    if (bootstrapped) return;
    bootstrapped = true;
    const isAndroid = untrack(() => root === "mobile" && androidBridgeAvailable());
    const start = untrack(() => initialPath);
    if (root !== "web") void untrack(() => load(start));
    if (!isAndroid) return;
    // Re-check the Android grant when the user returns from Settings.
    const onVisible = () => {
      if (document.visibilityState === "visible") void load(listing?.path ?? "");
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  });

  function crumbs(): Crumb[] {
    if (!listing?.path) return [];
    return friendlyCrumbs(listing.path);
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
        <button
          type="button"
          class="cell"
          onclick={() => openEntry(entry)}
          role="gridcell"
          use:lazyPreview={entry.path}
          data-preview-path={entry.path}
        >
          {#if entry.isDir}
            <div class="icon dir"><Icon name="folder" /></div>
          {:else}
            {@const kind = fileTypeIcon(entry.name)}
            {@const src = fileSrcUrl(entry.path)}
            {@const canPreview =
              (kind === "image" || kind === "video") &&
              !failedPreviews.has(entry.path) &&
              visiblePaths.has(entry.path) &&
              src !== null &&
              /* tracked so cells upgrade to previews once URLs warm up */
              srcVersion >= 0}
            {#if canPreview}
              <div class="tile">
                {#if kind === "image"}
                  <img
                    class="thumb"
                    {src}
                    alt=""
                    loading="lazy"
                    decoding="async"
                    onerror={() => markPreviewFailed(entry.path)}
                  />
                {:else}
                  <!-- svelte-ignore a11y_media_has_caption -->
                  <video
                    class="thumb"
                    {src}
                    preload="metadata"
                    muted
                    playsinline
                    onerror={() => markPreviewFailed(entry.path)}
                  ></video>
                {/if}
                <span class="ext">{extLabel(entry.name)}</span>
              </div>
            {:else}
              <div class="icon ftype">
                <Icon name={kind} size={40} strokeWidth={1.6} />
                <span class="ext">{extLabel(entry.name)}</span>
              </div>
            {/if}
          {/if}
          <span class="label">{entry.name}</span>
          {#if !entry.isDir}<span class="size">{sizeLabel(entry.size)}</span>{/if}
        </button>
      {/each}
    </div>
    {#if listing.hasMore}
      <div class="more-sentinel" bind:this={moreSentinel} aria-hidden="true">
        {#if loadingMore}
          <p class="status">Loading more…</p>
        {/if}
      </div>
    {/if}
    {#if listing.entries.length === 0 && !listing.hasMore}
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
  .icon.ftype {
    color: var(--text-secondary);
    position: relative;
    justify-content: center;
  }
  .icon.ftype :global(svg) {
    opacity: 0.9;
  }
  .icon.ftype .ext {
    position: absolute;
    right: 0.4rem;
    bottom: 0.4rem;
    font-family: ui-monospace, monospace;
    font-size: 0.55rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--text-primary);
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: 0.25rem;
    padding: 0.05rem 0.28rem;
    max-width: 70%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Live previews fill the tile; the ext badge overlays like a file manager. */
  .tile {
    position: relative;
    height: 96px;
    border-radius: 0.3rem;
    overflow: hidden;
    background: var(--bg-secondary);
  }
  .thumb {
    width: 100%;
    height: 96px;
    object-fit: cover;
    display: block;
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
  .more-sentinel {
    min-height: 56px;
    display: grid;
    place-items: center;
  }

  .perm-hint {
    font-size: 0.75rem;
  }
</style>
