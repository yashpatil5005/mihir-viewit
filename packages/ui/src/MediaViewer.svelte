<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { debugLog, openWithExternal } from '@viewit/platform';
  import RuntimeChooser from './RuntimeChooser.svelte';
  import type { PluginInfo } from './pluginBridge';

  let {
    uri,
    asset_path = '',
    media_kind = 'video',
    name = 'media',
    format = 'video',
    ext = 'mp4',
    stream_url = '',
  }: {
    uri: string;
    asset_path?: string;
    media_kind?: 'video' | 'audio';
    name?: string;
    format?: string;
    ext?: string;
    stream_url?: string;
  } = $props();

  let src = $state('');
  let errorMsg = $state('');
  let showExternalBtn = $state(false);
  let mediaEl = $state<HTMLVideoElement | HTMLAudioElement | null>(null);
  let loadStart = 0;
  let blobUrl = $state('');
  let currentStrategy = $state('');
  let nativePlayerLaunched = $state(false);
  let isAndroidTauri = $state(false);
  let runtimeChooserOpen = $state(false);

  const MIME_MAP: Record<string, string> = {
    mp4: 'video/mp4', m4v: 'video/mp4', webm: 'video/webm', mkv: 'video/x-matroska',
    mov: 'video/quicktime', '3gp': 'video/3gpp', avi: 'video/x-msvideo',
    mpg: 'video/mpeg', mpeg: 'video/mpeg', wmv: 'video/x-ms-wmv', flv: 'video/x-flv',
    ts: 'video/mp2t', m2ts: 'video/mp2t', mts: 'video/mp2t',
    ogv: 'video/ogg', f4v: 'video/mp4', asf: 'video/x-ms-wmv',
    mp3: 'audio/mpeg', m4a: 'audio/mp4', aac: 'audio/aac', flac: 'audio/flac',
    ogg: 'audio/ogg', wav: 'audio/wav', wma: 'audio/x-ms-wma', opus: 'audio/opus',
  };

  function mimeForExt(e: string): string {
    return MIME_MAP[e.toLowerCase()] ?? (media_kind === 'video' ? 'video/mp4' : 'audio/mpeg');
  }

  async function tryStreamProtocol(): Promise<boolean> {
    const { debugLog: log } = await import('@viewit/platform');
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const streamUrl = await invoke<string>('media_stream_url', { assetPath: asset_path, ext: ext || 'mp4' });
      log(`[media] stream URL: ${streamUrl}`);
      if (streamUrl) { src = streamUrl; currentStrategy = 'stream'; return true; }
    } catch (e) {
      log(`[media] stream protocol failed: ${e instanceof Error ? e.message : String(e)}`);
    }
    return false;
  }

  async function tryConvertFileSrc(): Promise<boolean> {
    const { debugLog: log } = await import('@viewit/platform');
    try {
      const { convertFileSrc } = await import('@tauri-apps/api/core');
      const filePath = asset_path.startsWith('file://') ? asset_path.slice(7) : asset_path;
      const converted = convertFileSrc(filePath);
      log(`[media] convertFileSrc → ${converted?.slice(0, 120)}`);
      if (converted) { src = converted; currentStrategy = 'convertFileSrc'; return true; }
    } catch (e) {
      log(`[media] convertFileSrc failed: ${e instanceof Error ? e.message : String(e)}`);
    }
    return false;
  }

  async function tryBlobUrl(): Promise<boolean> {
    const { debugLog: log } = await import('@viewit/platform');
    try {
      log(`[media] reading bytes via IPC…`);
      const { readMaterializedBytes } = await import('@viewit/platform');
      const uint8 = await readMaterializedBytes(asset_path);
      log(`[media] got ${uint8.length} bytes`);
      const mime = mimeForExt(ext);
      const blob = new Blob([uint8], { type: mime });
      blobUrl = URL.createObjectURL(blob);
      src = blobUrl;
      currentStrategy = 'blob';
      log(`[media] blob URL created, size=${blob.size}`);
      return true;
    } catch (e) {
      log(`[media] blob fallback failed: ${e instanceof Error ? e.message : String(e)}`);
    }
    return false;
  }

  async function launchNativePlayer() {
    const { debugLog: log } = await import('@viewit/platform');
    try {
      const videoUri = stream_url || uri;
      log(`[media] launching native player: ${videoUri?.slice(0, 80)}`);
      if ('AndroidBridge' in window) {
        (window as any).AndroidBridge.launchVideoPlayer(videoUri, name, ext || '');
        nativePlayerLaunched = true;
      } else {
        log(`[media] AndroidBridge not available`);
      }
    } catch (e) {
      log(`[media] native player failed: ${e instanceof Error ? e.message : String(e)}`);
      nativePlayerLaunched = false;
    }
  }

  async function useBuiltInRuntime() {
    if (isAndroidTauri && media_kind === 'video') {
      await launchNativePlayer();
      return;
    }
    await useWebRuntime();
  }

  async function useInstalledPlugin(_plugin: PluginInfo) {
    await launchNativePlayer();
  }

  async function useWebRuntime() {
    const { debugLog: log } = await import('@viewit/platform');
    if (stream_url) {
      log(`[media] using direct stream_url`);
      src = stream_url;
      currentStrategy = 'stream_url';
      return;
    }

    if (!asset_path) {
      errorMsg = 'No asset path — materialization may have failed';
      return;
    }

    const IS_TAURI = '__TAURI_INTERNALS__' in window;
    if (!IS_TAURI) { await tryBlobUrl(); return; }

    if (!(await tryStreamProtocol())) {
      if (!(await tryConvertFileSrc())) {
        await tryBlobUrl();
      }
    }
    log(`[media] src set via ${currentStrategy}, waiting for load…`);
  }

  onMount(async () => {
    const { debugLog: log } = await import('@viewit/platform');

    isAndroidTauri = '__TAURI_INTERNALS__' in window;

    log(`[media] uri=${uri?.slice(0, 80)}`);
    log(`[media] asset_path=${asset_path?.slice(0, 80)}`);
    log(`[media] stream_url=${stream_url?.slice(0, 80)}`);
    log(`[media] kind=${media_kind} format=${format} ext=${ext}`);

    loadStart = Date.now();

    // On Android/Tauri: let the user choose built-in/native/plugin/external.
    if (isAndroidTauri && media_kind === 'video') {
      runtimeChooserOpen = true;
      return;
    }
    await useWebRuntime();
  });

  onDestroy(() => {
    if (blobUrl) URL.revokeObjectURL(blobUrl);
  });

  function onLoad() {
    const elapsed = Date.now() - loadStart;
    debugLog(`[media] loaded in ${elapsed}ms via ${currentStrategy}`);
  }

  async function onError() {
    const el = mediaEl;
    if (!el) return;
    const err = (el as HTMLVideoElement).error;
    const code = err?.code ?? 0;
    const NAMES: Record<number, string> = { 1: 'ABORTED', 2: 'NETWORK', 3: 'DECODE', 4: 'SRC_NOT_SUPPORTED' };
    const msg = err?.message || NAMES[code] || `code ${code}`;
    debugLog(`[media] ERROR code=${code} (${NAMES[code] ?? '?'}): ${msg} strategy=${currentStrategy}`);

    src = '';

    if (code === 4) {
      if (currentStrategy === 'stream_url' || currentStrategy === 'stream' || currentStrategy === 'convertFileSrc') {
        debugLog(`[media] codec unsupported (${ext}), offering external player`);
        errorMsg = `Format .${ext} isn't supported by the built-in player.`;
        showExternalBtn = true;
        return;
      }
    }

    if (currentStrategy === 'convertFileSrc' && code === 4) {
      debugLog(`[media] convertFileSrc failed, trying blob…`);
      if (await tryBlobUrl()) { loadStart = Date.now(); return; }
    }

    if (currentStrategy === 'stream' && code === 4) {
      debugLog(`[media] stream failed, trying convertFileSrc…`);
      if (await tryConvertFileSrc()) { loadStart = Date.now(); return; }
      debugLog(`[media] convertFileSrc failed, trying blob…`);
      if (await tryBlobUrl()) { loadStart = Date.now(); return; }
    }

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
  function onWaiting() { debugLog(`[media] WAITING`); }
  function onEnded() { debugLog(`[media] ENDED`); }
</script>

<article class="media-viewer">
  <aside class="meta">
    <strong>{name}</strong>
    <span class="tag">{format}</span>
    {#if currentStrategy}
      <span class="hint">{currentStrategy}</span>
    {/if}
  </aside>
  <div class="frame">
    {#if nativePlayerLaunched}
      <p class="status">Playing in native player — press back to return</p>
    {:else if errorMsg}
      <p class="err">{errorMsg}</p>
      {#if showExternalBtn}
        <button class="external-btn" onclick={() => openWithExternal(uri)}>
          Open with another app…
        </button>
      {/if}
    {:else if !src}
      <p class="status">Loading…</p>
    {:else if media_kind === 'audio'}
      <audio bind:this={mediaEl} controls src={src} onerror={onError} onload={onLoad}></audio>
    {:else}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video bind:this={mediaEl} controls playsinline preload="auto" src={src}
        onerror={onError} onload={onLoad} onprogress={onProgress}
        onwaiting={onWaiting} onended={onEnded}
      ></video>
    {/if}
  </div>
</article>

<RuntimeChooser
  open={runtimeChooserOpen}
  uri={stream_url || uri}
  {name}
  {ext}
  builtInLabel="Native Android player"
  onClose={() => runtimeChooserOpen = false}
  onUseBuiltIn={useBuiltInRuntime}
  onUseInstalledPlugin={useInstalledPlugin}
/>

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
  .external-btn {
    margin-top: 0.75rem; padding: 0.5rem 1.2rem;
    background: var(--link); color: #fff; border: none; border-radius: 0.4rem;
    font-size: 0.85rem; cursor: pointer; font-weight: 500;
  }
  .external-btn:hover { opacity: 0.9; }
</style>
