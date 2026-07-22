<script lang="ts">
  import { onDestroy } from 'svelte';
  import { readMaterializedBytes, resolvePptxAssetPath, debugLog } from '@viewit/platform';

  let {
    document: docProp = {},
    source_uri = '',
  }: {
    document?: { asset_path?: string; slides?: Array<{ title: string; body: string }> };
    source_uri?: string;
  } = $props();

  let canvasEl: HTMLCanvasElement | null = $state(null);
  let rootEl: HTMLDivElement | null = $state(null);
  let status = $state<'loading' | 'ready' | 'error'>('loading');
  let errorMsg = $state('');
  let idx = $state(0);
  let total = $state(0);
  let preParsedSlides: Array<{ title: string; body: string }> = $state([]);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let viewer: any = null;

  async function loadDeck() {
    status = 'loading';
    errorMsg = '';
    try {
      // If the Rust backend already parsed slides (ODP, ODP fallback, etc.),
      // use them directly instead of re-parsing the raw file with pptx-parser.
      if (docProp.slides && docProp.slides.length > 0) {
        preParsedSlides = docProp.slides;
        total = docProp.slides.length;
        idx = 0;
        status = 'ready';
        debugLog(`pptx ok slides=${total} (pre-parsed)`);
        return;
      }

      const path = await resolvePptxAssetPath(
        docProp.asset_path,
        source_uri || undefined,
        null,
      );
      const bytes = await readMaterializedBytes(path);
      debugLog(`pptx bytes ${bytes.length}`);

      const { PptxViewer: OoxmlPptxViewer } = await import('@silurus/ooxml/pptx');

      if (!canvasEl) {
        status = 'error';
        errorMsg = 'Canvas not ready';
        return;
      }

      viewer = new OoxmlPptxViewer(canvasEl);
      await viewer.load(bytes);

      total = viewer.slideCount ?? 0;
      idx = viewer.currentSlide ?? 0;
      status = 'ready';
      debugLog(`pptx ok slides=${total}`);
    } catch (e) {
      status = 'error';
      errorMsg = e instanceof Error ? e.message : String(e);
      debugLog(`pptx err: ${errorMsg}`);
      viewer = null;
    }
  }

  $effect(() => {
    const uri = source_uri;
    const ap = docProp.asset_path;
    if (!uri && !ap) {
      status = 'error';
      errorMsg = 'Open a .pptx with Open with or Open file…';
      return;
    }
    if (!canvasEl) return;
    void loadDeck();
  });

  onDestroy(() => {
    viewer?.destroy?.();
    viewer = null;
  });

  function nav(d: number) {
    if (!viewer) return;
    const n = Math.max(0, Math.min(total - 1, idx + d));
    if (n !== idx) {
      idx = n;
      viewer.goToSlide(n);
    }
  }
</script>

<div class="pptx-root" bind:this={rootEl}>
  {#if status === 'loading'}<p class="muted">Loading presentation…</p>{/if}
  {#if status === 'error'}<p class="err">{errorMsg}</p>{/if}
  {#if status === 'ready'}
    <div class="bar">
      <button type="button" onclick={() => nav(-1)} disabled={idx <= 0}>←</button>
      <span>{idx + 1} / {total}</span>
      <button type="button" onclick={() => nav(1)} disabled={idx >= total - 1}>→</button>
    </div>
  {/if}
  <div class="stage">
    {#if preParsedSlides.length > 0}
      <div class="slide-text">
        {#if preParsedSlides[idx]}
          {#if preParsedSlides[idx].title}<h3>{preParsedSlides[idx].title}</h3>{/if}
          <pre>{preParsedSlides[idx].body}</pre>
        {/if}
      </div>
    {:else}
      <canvas bind:this={canvasEl}></canvas>
    {/if}
  </div>
  {#if status === 'ready'}
    <p class="muted">Offline · rendered via Canvas (no PowerPoint animations).</p>
  {/if}
</div>

<style>
  .pptx-root { padding: 0.5rem; color: var(--text-primary); }
  .bar { display: flex; gap: 0.5rem; align-items: center; margin-bottom: 0.5rem; flex-wrap: wrap; }
  .bar button { padding: 0.35rem 0.6rem; border: 1px solid var(--border); border-radius: 0.3rem; background: var(--bg-secondary); color: var(--text-primary); }
  .bar button:disabled { opacity: 0.35; }
  .stage {
    width: 100%; aspect-ratio: 16/9; max-height: 70vh; min-height: 200px;
    overflow: hidden; border: 1px solid var(--border); border-radius: 8px;
    background: #111; display: flex; align-items: center; justify-content: center;
  }
  canvas { max-width: 100%; max-height: 100%; display: block; }
  .slide-text {
    width: 100%; height: 100%; padding: 1.5rem;
    overflow: auto; color: var(--text-primary); background: var(--bg-primary);
    font-size: 0.9rem; line-height: 1.6;
  }
  .slide-text h3 { margin: 0 0 0.5rem; font-size: 1.1rem; }
  .slide-text pre { white-space: pre-wrap; word-break: break-word; margin: 0; font-family: inherit; }
  .muted { font-size: 0.75rem; color: var(--text-secondary); }
  .err { color: var(--error); white-space: pre-wrap; }
</style>
