<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { pdfPage, debugLog } from "@viewit/platform";

  type PromiseResolvers = {
    withResolvers?<T>(): {
      promise: Promise<T>;
      resolve: (value: T | PromiseLike<T>) => void;
      reject: (reason?: unknown) => void;
    };
  };
  // Polyfill Promise.withResolvers for older WebView versions (ES2024 feature).
  const PromiseCtor = Promise as PromiseConstructor & PromiseResolvers;
  if (typeof PromiseCtor.withResolvers !== "function") {
    PromiseCtor.withResolvers = function <T>() {
      let resolve!: (value: T | PromiseLike<T>) => void;
      let reject!: (reason?: unknown) => void;
      const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
      });
      return { promise, resolve, reject };
    };
  }

  let {
    document: docProp = {},
    source_uri = "",
  }: {
    document?: {
      page_count?: number;
      pages?: string[];
      byte_len?: number;
      native?: boolean;
      name?: string;
      asset_path?: string;
      stream_url?: string;
    };
    source_uri?: string;
  } = $props();

  let native = $derived(
    !!docProp.native ||
      !!(docProp as { asset_path?: string }).asset_path ||
      !!(docProp as { stream_url?: string }).stream_url,
  );
  let cachePath = $derived((docProp as { asset_path?: string }).asset_path ?? "");
  let streamUrl = $derived((docProp as { stream_url?: string }).stream_url ?? "");
  let page_count = $derived(docProp.page_count ?? 0);
  let byte_len = $derived(docProp.byte_len ?? 0);

  let pages = $state<string[]>([]);
  let loadingPage = $state<number | null>(null);
  let pdfjsDocument = $state<any>(null);
  let pdfjsPageCount = $state(0);
  let pdfjsPages = $state<Record<number, HTMLCanvasElement>>({});
  let thumbnailPages = $state<Record<number, HTMLCanvasElement>>({});
  let renderingPages = new Set<string>();
  let showThumbnails = $state(true);
  let currentPage = $state(1);
  let pdfjsStatus = $state<"idle" | "loading" | "error" | "ready">("idle");
  let pdfjsError = $state("");
  let pdfBlobUrl = $state("");
  let pdfStarted = false;

  $effect(() => {
    pages = [...(docProp.pages ?? [])];
  });

  function kickOffPdf() {
    if (pdfStarted || !native) return;
    pdfStarted = true;
    pdfjsStatus = "loading";

    // Web (no Tauri): `source_uri` is a `blob:`/`data:` URL the frontend made.
    // pdf.js reads it directly — no Tauri IPC involved.
    if (!("__TAURI_INTERNALS__" in window) && /^(blob:|data:|https?:)/.test(source_uri)) {
      debugLog(`[pdf] web: using blob source_uri ${source_uri.slice(0, 60)}…`);
      pdfBlobUrl = source_uri;
      void loadPdfJsAndRender();
      return;
    }

    // Priority 1: Direct stream URL (new architecture)
    if (streamUrl) {
      debugLog(`[pdf] using stream_url: ${streamUrl.slice(0, 80)}`);
      renderNativePdfJsFromStream(streamUrl);
      return;
    }

    // Fallback: cached file path
    if (cachePath) {
      debugLog(`[pdf] using cachePath: ${cachePath.slice(0, 80)}`);
      renderNativePdfJs(cachePath);
    }
  }

  // Fallback: if onMount fired before props were set, pick up when they arrive.
  $effect(() => {
    const _cp = cachePath;
    const _n = native;
    if (_n && _cp) kickOffPdf();
  });

  function bytesArrayBuffer(bytes: Uint8Array): ArrayBuffer {
    return Uint8Array.from(bytes).buffer;
  }

  async function renderNativePdfJsFromStream(url: string) {
    pdfjsStatus = "loading";
    pdfjsError = "";
    pdfjsPages = {};
    thumbnailPages = {};
    debugLog(`[pdf] renderNativePdfJsFromStream url=${url.slice(0, 80)}`);
    try {
      // Bytes over Tauri IPC — cross-origin JS fetch to the localhost stream
      // server is blocked in the Android WebView (secure context). Prefer the
      // app-private cache file (permission-independent) when materialized.
      const { readUriBytes, readMaterializedBytes } = await import("@viewit/platform");
      const uint8 = cachePath
        ? await readMaterializedBytes(cachePath)
        : await readUriBytes(source_uri);
      const blob = new Blob([bytesArrayBuffer(uint8)], { type: "application/pdf" });
      pdfBlobUrl = URL.createObjectURL(blob);
      debugLog(`[pdf] blob url created from bytes, size=${uint8.length}`);

      await loadPdfJsAndRender();
    } catch (e) {
      pdfjsStatus = "error";
      pdfjsError = e instanceof Error ? e.message : String(e);
      debugLog(`[pdf] ERROR: ${pdfjsError}`);
    }
  }

  async function renderNativePdfJs(path: string) {
    pdfjsStatus = "loading";
    pdfjsError = "";
    pdfjsPages = {};
    thumbnailPages = {};
    debugLog(`[pdf] renderNativePdfJs path=${path.slice(0, 80)}`);
    try {
      const { readMaterializedBytes } = await import("@viewit/platform");
      debugLog(`[pdf] readMaterializedBytes calling…`);
      const uint8 = await readMaterializedBytes(path);
      debugLog(`[pdf] got ${uint8.length} bytes`);
      const blob = new Blob([bytesArrayBuffer(uint8)], { type: "application/pdf" });
      pdfBlobUrl = URL.createObjectURL(blob);
      debugLog(`[pdf] blob url created, size=${blob.size}`);

      await loadPdfJsAndRender();
    } catch (e) {
      pdfjsStatus = "error";
      pdfjsError = e instanceof Error ? e.message : String(e);
      debugLog(`[pdf] ERROR: ${pdfjsError}`);
    }
  }

  async function loadPdfJsAndRender() {
    let pdfjs: any;
    try {
      pdfjs = await import("pdfjs-dist");
      debugLog(`[pdf] pdfjs-dist loaded`);
    } catch (e) {
      throw new Error(`Failed to load pdf.js: ${e instanceof Error ? e.message : e}`);
    }

    // Set up the worker. pdfjs-dist v4+ requires a worker — cannot disable.
    // The ?url import resolves to a local asset URL that the WebView can load.
    // We wrap the worker to inject a Promise.withResolvers polyfill for older WebViews.
    try {
      const workerUrl = await import("pdfjs-dist/build/pdf.worker.mjs?url").then((m) => m.default);
      debugLog(`[pdf] worker src set: ${String(workerUrl).slice(0, 80)}`);

      // Fetch the worker source and prepend the polyfill
      const workerResp = await fetch(workerUrl);
      const workerCode = await workerResp.text();
      const polyfill = `
        if (typeof Promise.withResolvers !== 'function') {
          Promise.withResolvers = function() {
            var resolve, reject;
            var promise = new Promise(function(res, rej) { resolve = res; reject = rej; });
            return { promise: promise, resolve: resolve, reject: reject };
          };
        }
      `;
      const wrappedBlob = new Blob([polyfill + workerCode], { type: "application/javascript" });
      pdfjs.GlobalWorkerOptions.workerSrc = URL.createObjectURL(wrappedBlob);
    } catch (e) {
      debugLog(
        `[pdf] worker import failed: ${e instanceof Error ? e.message : e}, falling back to null`,
      );
      pdfjs.GlobalWorkerOptions.workerSrc = "";
    }

    const task = pdfjs.getDocument({ url: pdfBlobUrl, disableRange: false });
    const pdf = await task.promise;
    debugLog(`[pdf] pdf loaded, numPages=${pdf.numPages}`);
    pdfjsDocument = pdf;
    pdfjsPageCount = pdf.numPages;
    pdfjsStatus = "ready";
  }

  async function renderPdfPage(num: number, thumbnail: boolean) {
    const key = `${thumbnail ? "thumb" : "page"}-${num}`;
    const cache = thumbnail ? thumbnailPages : pdfjsPages;
    if (!pdfjsDocument || cache[num] || renderingPages.has(key)) return;
    renderingPages.add(key);
    try {
      const page = await pdfjsDocument.getPage(num);
      const unscaled = page.getViewport({ scale: 1 });
      const scale = thumbnail
        ? Math.min(140 / unscaled.width, 180 / unscaled.height)
        : Math.min(2048 / unscaled.width, 2048 / unscaled.height, 1.5);
      const viewport = page.getViewport({ scale });
      const canvas = document.createElement("canvas");
      canvas.width = Math.ceil(viewport.width);
      canvas.height = Math.ceil(viewport.height);
      const context = canvas.getContext("2d");
      if (!context) return;
      await page.render({ canvasContext: context, viewport }).promise;
      if (thumbnail) thumbnailPages = { ...thumbnailPages, [num]: canvas };
      else pdfjsPages = { ...pdfjsPages, [num]: canvas };
    } catch (e) {
      debugLog(`[pdf] page ${num} render failed: ${e instanceof Error ? e.message : e}`);
    } finally {
      renderingPages.delete(key);
    }
  }

  function lazyPdfPage(node: HTMLElement, params: { num: number; thumbnail: boolean }) {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          void renderPdfPage(params.num, params.thumbnail);
          observer.disconnect();
        }
      },
      { rootMargin: params.thumbnail ? "300px" : "800px" },
    );
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  function jumpToPage(num: number) {
    currentPage = num;
    document
      .getElementById(`pdf-page-${num}`)
      ?.scrollIntoView({ behavior: "smooth", block: "start" });
    void renderPdfPage(num, false);
  }

  async function ensurePage(i: number) {
    if (pages[i] || !source_uri || loadingPage !== null) return;
    loadingPage = i;
    try {
      const url = await pdfPage(source_uri, i);
      const next = [...pages];
      while (next.length <= i) next.push("");
      next[i] = url;
      pages = next;
    } finally {
      loadingPage = null;
    }
  }

  let containerEl: HTMLElement | null = $state(null);

  function onKey(e: KeyboardEvent) {
    if (!containerEl) return;
    const pageEls = Array.from(containerEl.querySelectorAll(".page"));
    if (pageEls.length === 0) return;
    const scrollY = window.scrollY;
    let cur = 0;
    for (let i = 0; i < pageEls.length; i++) {
      if ((pageEls[i] as HTMLElement).offsetTop > scrollY) {
        cur = i;
        break;
      }
    }
    let next = cur;
    if (e.key === "ArrowDown" || e.key === "PageDown") next = Math.min(cur + 1, pageEls.length - 1);
    else if (e.key === "ArrowUp" || e.key === "PageUp") next = Math.max(cur - 1, 0);
    else return;
    e.preventDefault();
    (pageEls[next] as HTMLElement).scrollIntoView({ block: "start", behavior: "smooth" });
  }

  function mountCanvas(node: HTMLElement, canvas: HTMLCanvasElement) {
    node.replaceChildren(canvas);
    return {
      destroy() {
        node.replaceChildren();
      },
    };
  }

  onMount(() => {
    window.addEventListener("keydown", onKey);
    // Primary trigger — matches MediaViewer's proven onMount pattern.
    kickOffPdf();
  });
  onDestroy(() => {
    window.removeEventListener("keydown", onKey);
    if (pdfBlobUrl) URL.revokeObjectURL(pdfBlobUrl);
  });
</script>

<article class="pdf-viewer">
  <aside class="meta">
    {#if native}
      <strong>{docProp.name ?? "PDF"}</strong> · pdf.js · {byte_len.toLocaleString()} bytes
    {:else}
      <strong>{page_count} page{page_count !== 1 ? "s" : ""}</strong> · {byte_len.toLocaleString()} bytes
      · pdf
    {/if}
    {#if native && pdfjsPageCount > 0}
      <button class="thumbnail-toggle" onclick={() => (showThumbnails = !showThumbnails)}>
        {showThumbnails ? "Hide" : "Show"} thumbnails
      </button>
    {/if}
  </aside>
  {#if native}
    <div class="native-layout" class:with-thumbnails={showThumbnails && pdfjsPageCount > 0}>
      {#if showThumbnails && pdfjsPageCount > 0}
        <nav class="thumbnail-sidebar" aria-label="PDF pages">
          {#each Array(pdfjsPageCount) as _, i}
            <button
              class="thumbnail"
              class:active={currentPage === i + 1}
              onclick={() => jumpToPage(i + 1)}
              aria-label="Go to page {i + 1}"
              use:lazyPdfPage={{ num: i + 1, thumbnail: true }}
            >
              <span class="thumbnail-canvas">
                {#if thumbnailPages[i + 1]}
                  <span use:mountCanvas={thumbnailPages[i + 1]}></span>
                {:else}
                  <span class="thumbnail-placeholder"></span>
                {/if}
              </span>
              <span>{i + 1}</span>
            </button>
          {/each}
        </nav>
      {/if}
      <div class="native-frame" bind:this={containerEl}>
        {#if pdfjsStatus === "loading"}
          <p class="status">Loading PDF…</p>
        {:else if pdfjsStatus === "error"}
          <p class="error">{pdfjsError}</p>
        {:else if pdfjsPageCount > 0}
          {#each Array(pdfjsPageCount) as _, i}
            <figure
              class="page native-page"
              id="pdf-page-{i + 1}"
              use:lazyPdfPage={{ num: i + 1, thumbnail: false }}
            >
              <figcaption>Page {i + 1}</figcaption>
              {#if pdfjsPages[i + 1]}
                <div class="canvas-wrap" use:mountCanvas={pdfjsPages[i + 1]}></div>
              {:else}
                <div class="page-placeholder">Rendering page {i + 1}…</div>
              {/if}
            </figure>
          {/each}
        {/if}
      </div>
    </div>
  {:else}
    <div class="pages" bind:this={containerEl} role="document">
      {#each Array(page_count) as _, i}
        <figure class="page">
          <figcaption>Page {i + 1}</figcaption>
          {#if pages[i]}
            <img src={pages[i]} alt="Page {i + 1}" loading="lazy" />
          {:else}
            <button
              type="button"
              class="load-page"
              onclick={() => ensurePage(i)}
              disabled={loadingPage === i}
            >
              {loadingPage === i ? "Rendering…" : "Load page"}
            </button>
          {/if}
        </figure>
      {/each}
    </div>
  {/if}
</article>

<style>
  .pdf-viewer {
    padding: 0.5rem 1rem;
  }
  .meta {
    color: var(--text-secondary);
    margin-bottom: 1rem;
    font-size: 0.75rem;
  }
  .thumbnail-toggle {
    margin-left: 0.75rem;
    color: var(--link);
    background: none;
    border: 0;
    cursor: pointer;
    font: inherit;
  }
  .native-layout.with-thumbnails {
    display: grid;
    grid-template-columns: 10rem minmax(0, 1fr);
    gap: 1rem;
    align-items: start;
  }
  .thumbnail-sidebar {
    position: sticky;
    top: 0.5rem;
    max-height: calc(100vh - 2rem);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--bg-secondary);
  }
  .thumbnail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    padding: 0.35rem;
    border: 1px solid transparent;
    border-radius: 0.35rem;
    color: var(--text-secondary);
    background: transparent;
    cursor: pointer;
  }
  .thumbnail.active {
    border-color: var(--link);
    color: var(--text-primary);
  }
  .thumbnail-canvas,
  .thumbnail-canvas :global(canvas),
  .thumbnail-placeholder {
    display: block;
    max-width: 100%;
  }
  .thumbnail-placeholder {
    width: 7rem;
    height: 9rem;
    background: color-mix(in srgb, var(--text-secondary) 12%, transparent);
  }
  .native-page {
    width: 100%;
    min-height: 28rem;
    scroll-margin-top: 1rem;
  }
  .page-placeholder {
    width: min(100%, 40rem);
    min-height: 28rem;
    display: grid;
    place-items: center;
    color: var(--text-secondary);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
  }
  .pages,
  .native-frame {
    display: flex;
    flex-direction: column;
    gap: 2rem;
    align-items: center;
  }
  .page {
    max-width: 100%;
  }
  .page figcaption {
    text-align: center;
    color: var(--text-secondary);
    font-size: 0.8rem;
    margin-bottom: 0.5rem;
  }
  .page img,
  .canvas-wrap :global(canvas) {
    max-width: 100%;
    height: auto;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }
  .status {
    color: var(--text-secondary);
    font-style: italic;
    padding: 1rem;
  }
  .error {
    color: var(--error);
    padding: 1rem;
    white-space: pre-wrap;
  }
  .load-page {
    padding: 0.5rem 1rem;
    cursor: pointer;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
  }
  @media (max-width: 700px) {
    .native-layout.with-thumbnails {
      grid-template-columns: 1fr;
    }
    .thumbnail-sidebar {
      position: sticky;
      z-index: 2;
      flex-direction: row;
      max-height: none;
      overflow-x: auto;
    }
    .thumbnail {
      min-width: 5rem;
    }
    .thumbnail-canvas,
    .thumbnail-placeholder {
      display: none;
    }
  }
</style>
