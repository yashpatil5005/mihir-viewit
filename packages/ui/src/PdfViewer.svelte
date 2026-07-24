<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { pdfPage, debugLog } from '@viewit/platform';

  // Polyfill Promise.withResolvers for older WebView versions (ES2024 feature)
  // pdfjs-dist v4 uses this internally, so we must polyfill before importing it.
  if (typeof Promise.withResolvers !== 'function') {
    Promise.withResolvers = function <T>() {
      let resolve!: (value: T | PromiseLike<T>) => void;
      let reject!: (reason?: any) => void;
      const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
      return { promise, resolve, reject };
    };
  }

  let {
    document: docProp = {},
    source_uri = '',
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

  let native = $derived(!!docProp.native || !!(docProp as { asset_path?: string }).asset_path || !!(docProp as { stream_url?: string }).stream_url);
  let cachePath = $derived((docProp as { asset_path?: string }).asset_path ?? '');
  let streamUrl = $derived((docProp as { stream_url?: string }).stream_url ?? '');
  let page_count = $derived(docProp.page_count ?? 0);
  let byte_len = $derived(docProp.byte_len ?? 0);

  let pages = $state<string[]>([]);
  let loadingPage = $state<number | null>(null);
  let pdfjsPages = $state<{ canvas: HTMLCanvasElement; num: number }[]>([]);
  let pdfjsStatus = $state<'idle' | 'loading' | 'error' | 'ready'>('idle');
  let pdfjsError = $state('');
  let pdfBlobUrl = $state('');
  let pdfStarted = false;

  $effect(() => {
    pages = [...(docProp.pages ?? [])];
  });

  function kickOffPdf() {
    if (pdfStarted || !native) return;
    pdfStarted = true;
    pdfjsStatus = 'loading';

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

  async function renderNativePdfJsFromStream(url: string) {
    pdfjsStatus = 'loading';
    pdfjsError = '';
    pdfjsPages = [];
    debugLog(`[pdf] renderNativePdfJsFromStream url=${url.slice(0, 80)}`);
    try {
      // Fetch PDF directly from stream server
      const response = await fetch(url);
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }
      const blob = await response.blob();
      pdfBlobUrl = URL.createObjectURL(blob);
      debugLog(`[pdf] blob url created from stream, size=${blob.size}`);

      await loadPdfJsAndRender();
    } catch (e) {
      pdfjsStatus = 'error';
      pdfjsError = e instanceof Error ? e.message : String(e);
      debugLog(`[pdf] ERROR: ${pdfjsError}`);
    }
  }

  async function renderNativePdfJs(path: string) {
    pdfjsStatus = 'loading';
    pdfjsError = '';
    pdfjsPages = [];
    debugLog(`[pdf] renderNativePdfJs path=${path.slice(0, 80)}`);
    try {
      const { readMaterializedBytes } = await import('@viewit/platform');
      debugLog(`[pdf] readMaterializedBytes calling…`);
      const uint8 = await readMaterializedBytes(path);
      debugLog(`[pdf] got ${uint8.length} bytes`);
      const blob = new Blob([uint8], { type: 'application/pdf' });
      pdfBlobUrl = URL.createObjectURL(blob);
      debugLog(`[pdf] blob url created, size=${blob.size}`);

      await loadPdfJsAndRender();
    } catch (e) {
      pdfjsStatus = 'error';
      pdfjsError = e instanceof Error ? e.message : String(e);
      debugLog(`[pdf] ERROR: ${pdfjsError}`);
    }
  }

  async function loadPdfJsAndRender() {
    let pdfjs: any;
    try {
      pdfjs = await import('pdfjs-dist');
      debugLog(`[pdf] pdfjs-dist loaded`);
    } catch (e) {
      throw new Error(`Failed to load pdf.js: ${e instanceof Error ? e.message : e}`);
    }

    // Set up the worker. pdfjs-dist v4+ requires a worker — cannot disable.
    // The ?url import resolves to a local asset URL that the WebView can load.
    // We wrap the worker to inject a Promise.withResolvers polyfill for older WebViews.
    try {
      const workerUrl = await import('pdfjs-dist/build/pdf.worker.mjs?url').then(m => m.default);
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
      const wrappedBlob = new Blob([polyfill + workerCode], { type: 'application/javascript' });
      pdfjs.GlobalWorkerOptions.workerSrc = URL.createObjectURL(wrappedBlob);
    } catch (e) {
      debugLog(`[pdf] worker import failed: ${e instanceof Error ? e.message : e}, falling back to null`);
      pdfjs.GlobalWorkerOptions.workerSrc = '';
    }

    const task = pdfjs.getDocument({ url: pdfBlobUrl, disableRange: false });
    const pdf = await task.promise;
    debugLog(`[pdf] pdf loaded, numPages=${pdf.numPages}`);
    const n = Math.min(pdf.numPages, 30);
    const MAX_DIM = 2048;
    const out: { canvas: HTMLCanvasElement; num: number }[] = [];
    try {
    for (let i = 1; i <= n; i++) {
      try {
        debugLog(`[pdf] getPage(${i})…`);
        const page = await pdf.getPage(i);
        const unscaled = page.getViewport({ scale: 1 });
        const rawScale = Math.min(MAX_DIM / unscaled.width, MAX_DIM / unscaled.height, 2);
        const scale = Math.min(rawScale, 1.5);
          const vp = page.getViewport({ scale });
          debugLog(`[pdf] viewport ${vp.width}x${vp.height} scale=${scale.toFixed(2)}`);
          const canvas = document.createElement('canvas');
          canvas.width = vp.width;
          canvas.height = vp.height;
          const ctx = canvas.getContext('2d');
          if (!ctx) { debugLog(`[pdf] no 2d ctx for page ${i}`); continue; }
          debugLog(`[pdf] rendering page ${i}…`);
          await page.render({ canvasContext: ctx, viewport: vp }).promise;
          out.push({ canvas, num: i });
          debugLog(`[pdf] rendered page ${i}/${n}`);
        } catch (pageErr) {
          debugLog(`[pdf] page ${i} render failed: ${pageErr instanceof Error ? pageErr.message : pageErr}`);
        }
      }
      pdfjsPages = out;
      pdfjsStatus = 'ready';
      debugLog(`[pdf] done — ${n} pages rendered`);
    } catch (e) {
      pdfjsStatus = 'error';
      pdfjsError = e instanceof Error ? e.message : String(e);
      debugLog(`[pdf] ERROR: ${pdfjsError}`);
    }
  }

  async function ensurePage(i: number) {
    if (pages[i] || !source_uri || loadingPage !== null) return;
    loadingPage = i;
    try {
      const url = await pdfPage(source_uri, i);
      const next = [...pages];
      while (next.length <= i) next.push('');
      next[i] = url;
      pages = next;
    } finally {
      loadingPage = null;
    }
  }

  let containerEl: HTMLElement | null = $state(null);

  function onKey(e: KeyboardEvent) {
    if (!containerEl) return;
    const pageEls = Array.from(containerEl.querySelectorAll('.page'));
    if (pageEls.length === 0) return;
    const scrollY = window.scrollY;
    let cur = 0;
    for (let i = 0; i < pageEls.length; i++) {
      if ((pageEls[i] as HTMLElement).offsetTop > scrollY) { cur = i; break; }
    }
    let next = cur;
    if (e.key === 'ArrowDown' || e.key === 'PageDown') next = Math.min(cur + 1, pageEls.length - 1);
    else if (e.key === 'ArrowUp' || e.key === 'PageUp') next = Math.max(cur - 1, 0);
    else return;
    e.preventDefault();
    (pageEls[next] as HTMLElement).scrollIntoView({ block: 'start', behavior: 'smooth' });
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
    window.addEventListener('keydown', onKey);
    // Primary trigger — matches MediaViewer's proven onMount pattern.
    kickOffPdf();
  });
  onDestroy(() => {
    window.removeEventListener('keydown', onKey);
    if (pdfBlobUrl) URL.revokeObjectURL(pdfBlobUrl);
  });
</script>

<article class="pdf-viewer">
  <aside class="meta">
    {#if native}
      <strong>{docProp.name ?? 'PDF'}</strong> · pdf.js · {byte_len.toLocaleString()} bytes
    {:else}
      <strong>{page_count} page{page_count !== 1 ? 's' : ''}</strong> · {byte_len.toLocaleString()} bytes · pdf
    {/if}
  </aside>
  {#if native}
    <div class="native-frame">
      {#if pdfjsStatus === 'loading'}
        <p class="status">Loading PDF…</p>
      {:else if pdfjsStatus === 'error'}
        <p class="error">{pdfjsError}</p>
      {:else if pdfjsPages.length > 0}
        {#each pdfjsPages as p (p.num)}
          <figure class="page">
            <figcaption>Page {p.num}</figcaption>
            <div class="canvas-wrap" use:mountCanvas={p.canvas}></div>
          </figure>
        {/each}
      {/if}
    </div>
  {:else}
  <div class="pages" bind:this={containerEl} role="document">
    {#each Array(page_count) as _, i}
      <figure class="page">
        <figcaption>Page {i + 1}</figcaption>
        {#if pages[i]}
          <img src={pages[i]} alt="Page {i + 1}" loading="lazy" />
        {:else}
          <button type="button" class="load-page" onclick={() => ensurePage(i)} disabled={loadingPage === i}>
            {loadingPage === i ? 'Rendering…' : 'Load page'}
          </button>
        {/if}
      </figure>
    {/each}
  </div>
  {/if}
</article>

<style>
  .pdf-viewer { padding: 0.5rem 1rem; }
  .meta { color: var(--text-secondary); margin-bottom: 1rem; font-size: 0.75rem; }
  .pages, .native-frame { display: flex; flex-direction: column; gap: 2rem; align-items: center; }
  .page { max-width: 100%; }
  .page figcaption { text-align: center; color: var(--text-secondary); font-size: 0.8rem; margin-bottom: 0.5rem; }
  .page img, .canvas-wrap :global(canvas) { max-width: 100%; height: auto; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }
  .status { color: var(--text-secondary); font-style: italic; padding: 1rem; }
  .error { color: var(--error); padding: 1rem; white-space: pre-wrap; }
  .load-page { padding: 0.5rem 1rem; cursor: pointer; background: var(--bg-secondary); border: 1px solid var(--border); border-radius: 0.4rem; }
</style>
