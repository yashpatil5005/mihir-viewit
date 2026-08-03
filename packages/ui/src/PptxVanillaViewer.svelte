<script lang="ts">
  import { onDestroy } from 'svelte';
  import { readMaterializedBytes, resolvePptxAssetPath, debugLog } from '@viewit/platform';
  import { loadJsPlugin, type PluginInfo } from './pluginBridge';

  let {
    document: docProp = {},
    source_uri = '',
    plugin = null,
  }: {
    document?: { asset_path?: string; name?: string };
    source_uri?: string;
    plugin?: PluginInfo | null;
  } = $props();

  let hostEl: HTMLDivElement | null = $state(null);
  let status = $state<'loading' | 'ready' | 'error'>('loading');
  let errorMsg = $state('');
  let slideIdx = $state(0);
  let slideCount = $state(0);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let viewer: any = null;

  async function loadDeck() {
    status = 'loading';
    errorMsg = '';
    slideIdx = 0;
    slideCount = 0;
    try {
      if (!plugin) throw new Error('Missing PPTX vanilla plugin');
      const path = await resolvePptxAssetPath(
        (docProp as any).asset_path,
        source_uri || undefined,
      );
      const bytes = await readMaterializedBytes(path);
      debugLog(`pptx-vanilla bytes ${bytes.byteLength}`);

      const mod = await loadJsPlugin(plugin);
      if (!mod || typeof mod.createPptxViewer !== 'function') {
        throw new Error(`${plugin.name} has no createPptxViewer export`);
      }
      if (!hostEl) {
        status = 'error';
        errorMsg = 'Host not ready';
        return;
      }

      viewer = mod.createPptxViewer(hostEl, {
        source: bytes,
        editable: false,
        showToolbar: false,
        showThumbnails: false,
        locale: 'en',
        initialSlide: 0,
        onLoad: (info: { slideCount: number }) => {
          slideCount = info.slideCount ?? 0;
          slideIdx = 0;
          status = 'ready';
          debugLog(`pptx-vanilla ready slides=${slideCount}`);
        },
        onSlideChange: (index: number) => {
          slideIdx = index;
        },
        onError: (message: string) => {
          status = 'error';
          errorMsg = message;
          debugLog(`pptx-vanilla err: ${message}`);
        },
      });
    } catch (e) {
      status = 'error';
      errorMsg = e instanceof Error ? e.message : String(e);
      debugLog(`pptx-vanilla err: ${errorMsg}`);
      viewer = null;
    }
  }

  $effect(() => {
    const uri = source_uri;
    const ap = (docProp as any).asset_path;
    if (!uri && !ap) {
      status = 'error';
      errorMsg = 'Open a .pptx with Open with or Open file…';
      return;
    }
    if (!hostEl) return;
    void loadDeck();
  });

  onDestroy(() => {
    try {
      viewer?.destroy?.();
    } catch { /* ignore */ }
    viewer = null;
  });

  function nav(d: number) {
    if (!viewer) return;
    if (d < 0) viewer.prev?.();
    else viewer.next?.();
  }
</script>

<div class="pvv-root">
  {#if status === 'loading'}<p class="muted">Loading presentation…</p>{/if}
  {#if status === 'error'}<p class="err">{errorMsg}</p>{/if}
  {#if status === 'ready'}
    <div class="bar">
      <button type="button" onclick={() => nav(-1)} disabled={slideIdx <= 0}>←</button>
      <span>{slideIdx + 1} / {slideCount || '…'}</span>
      <button type="button" onclick={() => nav(1)} disabled={slideCount ? slideIdx >= slideCount - 1 : true}>→</button>
    </div>
  {/if}
  <div class="stage" class:staged-ready={status === 'ready'} bind:this={hostEl}></div>
  {#if status === 'ready'}
    <p class="muted">Rendered by {plugin?.name ?? 'PPTX Vanilla'} · view-only (no PowerPoint animations, no editing).</p>
  {/if}
</div>

<style>
  .pvv-root { padding: 0.5rem; color: var(--text-primary); }
  .bar { display: flex; gap: 0.5rem; align-items: center; margin-bottom: 0.5rem; flex-wrap: wrap; }
  .bar button { padding: 0.35rem 0.6rem; border: 1px solid var(--border); border-radius: 0.3rem; background: var(--bg-secondary); color: var(--text-primary); }
  .bar button:disabled { opacity: 0.35; }
  .stage {
    width: 100%; aspect-ratio: 16/9; max-height: 70vh; min-height: 200px;
    overflow: hidden; border: 1px solid var(--border); border-radius: 8px;
    background: #111; display: flex; align-items: center; justify-content: center;
  }
  .muted { color: var(--text-secondary); font-size: 0.8rem; margin-top: 0.35rem; }
  .err { color: var(--danger, #ff6b6b); padding: 0.5rem; }
</style>
