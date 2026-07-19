<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { debugLog } from '@viewit/platform';

  let {
    uri,
    asset_path = '',
    media_kind = 'video',
    name = 'media',
    format = 'video',
  }: {
    uri: string;
    asset_path?: string;
    media_kind?: 'video' | 'audio';
    name?: string;
    format?: string;
  } = $props();

  let src = $state('');
  let errorMsg = $state('');
  let mediaEl = $state<HTMLVideoElement | HTMLAudioElement | null>(null);
  let loadStart = 0;
  let blobUrl = $state('');

  onMount(async () => {
    const { debugLog: log } = await import('@viewit/platform');

    log(`[media] uri=${uri?.slice(0, 80)}`);
    log(`[media] asset_path=${asset_path?.slice(0, 80)}`);
    log(`[media] kind=${media_kind} format=${format}`);

    if (!asset_path) {
      errorMsg = 'No asset path — materialization may have failed';
      log(`[media] ABORT: no asset_path`);
      return;
    }

    try {
      log(`[media] reading bytes via IPC…`);
      const { readMaterializedBytes } = await import('@viewit/platform');
      const uint8 = await readMaterializedBytes(asset_path);
      log(`[media] got ${uint8.length} bytes`);

      const mime = media_kind === 'video' ? 'video/mp4' : 'audio/mpeg';
      const blob = new Blob([uint8], { type: mime });
      blobUrl = URL.createObjectURL(blob);
      src = blobUrl;
      log(`[media] blob URL created, size=${blob.size}`);

      loadStart = Date.now();
      log(`[media] src set, waiting for load…`);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      log(`[media] error: ${msg}`);
      errorMsg = msg;
    }
  });

  onDestroy(() => {
    if (blobUrl) {
      URL.revokeObjectURL(blobUrl);
    }
  });

  function onLoad() {
    const elapsed = Date.now() - loadStart;
    debugLog(`[media] loaded in ${elapsed}ms`);
  }

  function onError() {
    const el = mediaEl;
    if (!el) return;
    const err = (el as HTMLVideoElement).error;
    const code = err?.code ?? 0;
    const NAMES: Record<number, string> = { 1: 'ABORTED', 2: 'NETWORK', 3: 'DECODE', 4: 'SRC_NOT_SUPPORTED' };
    const msg = err?.message || NAMES[code] || `code ${code}`;
    debugLog(`[media] ERROR code=${code} (${NAMES[code] ?? '?'}): ${msg}`);
    errorMsg = `Playback error: ${msg}`;
  }

  function onProgress() {
    const el = mediaEl as HTMLVideoElement | null;
    if (!el) return;
    const buf = el.buffered;
    if (buf.length > 0) {
      debugLog(`[media] buffered 0-${buf.end(buf.length - 1).toFixed(1)}s`);
    }
  }

  function onWaiting() {
    debugLog(`[media] WAITING`);
  }

  function onEnded() {
    debugLog(`[media] ENDED`);
  }
</script>

<article class="media-viewer">
  <aside class="meta">
    <strong>{name}</strong>
    <span class="tag">{format}</span>
    <span class="hint">Offline — loaded via IPC blob URL</span>
  </aside>
  <div class="frame">
    {#if errorMsg}
      <p class="err">{errorMsg}</p>
    {:else if !src}
      <p class="status">Loading…</p>
    {:else if media_kind === 'audio'}
      <audio
        bind:this={mediaEl}
        controls
        src={src}
        onerror={onError}
        onload={onLoad}
      ></audio>
    {:else}
      <video
        bind:this={mediaEl}
        controls
        playsinline
        preload="auto"
        src={src}
        onerror={onError}
        onload={onLoad}
        onprogress={onProgress}
        onwaiting={onWaiting}
        onended={onEnded}
      ></video>
    {/if}
  </div>
</article>

<style>
  .media-viewer { display: flex; flex-direction: column; height: 100%; min-height: 40vh; }
  .meta { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: baseline; padding: 0.5rem 1rem; border-bottom: 1px solid var(--border); font-size: 0.8rem; color: var(--text-secondary); }
  .meta strong { color: var(--text-primary); }
  .tag { font-family: ui-monospace, monospace; background: var(--bg-secondary); padding: 0.1rem 0.4rem; border-radius: 0.25rem; }
  .hint { font-size: 0.7rem; opacity: 0.85; }
  .frame { flex: 1; display: flex; align-items: center; justify-content: center; padding: 1rem; background: #000; }
  video { max-width: 100%; max-height: 70vh; }
  audio { width: min(100%, 32rem); }
  .status { color: var(--text-secondary); font-style: italic; }
  .err { color: var(--error); white-space: pre-wrap; text-align: center; padding: 1rem; font-size: 0.85rem; }
</style>
