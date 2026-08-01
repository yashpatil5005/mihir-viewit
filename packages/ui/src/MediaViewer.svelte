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
    aif: 'audio/aiff', aiff: 'audio/aiff',
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

  async function tryAiffWavUrl(): Promise<boolean> {
    if (ext !== 'aif' && ext !== 'aiff') return false;
    const { debugLog: log } = await import('@viewit/platform');
    try {
      const bytes = stream_url
        ? new Uint8Array(await (await fetch(stream_url)).arrayBuffer())
        : await (async () => {
            const { readMaterializedBytes } = await import('@viewit/platform');
            return readMaterializedBytes(asset_path);
          })();
      const wav = aiffToWav(bytes);
      blobUrl = URL.createObjectURL(new Blob([wav], { type: 'audio/wav' }));
      src = blobUrl;
      currentStrategy = 'aiff-wav';
      log(`[media] AIFF decoded to WAV bytes=${wav.length}`);
      return true;
    } catch (e) {
      log(`[media] AIFF decode failed: ${e instanceof Error ? e.message : String(e)}`);
      return false;
    }
  }

  function aiffToWav(bytes: Uint8Array): Uint8Array {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const ascii = (offset: number, len: number) => String.fromCharCode(...bytes.subarray(offset, offset + len));
    if (ascii(0, 4) !== 'FORM' || ascii(8, 4) !== 'AIFF') throw new Error('not AIFF');
    let channels = 0;
    let frames = 0;
    let bits = 0;
    let sampleRate = 0;
    let pcmStart = 0;
    let pcmLen = 0;
    for (let pos = 12; pos + 8 <= bytes.length;) {
      const id = ascii(pos, 4);
      const size = view.getUint32(pos + 4, false);
      const data = pos + 8;
      if (id === 'COMM') {
        channels = view.getUint16(data, false);
        frames = view.getUint32(data + 2, false);
        bits = view.getUint16(data + 6, false);
        sampleRate = readExtended80(view, data + 8);
      } else if (id === 'SSND') {
        const offset = view.getUint32(data, false);
        pcmStart = data + 8 + offset;
        pcmLen = size - 8 - offset;
      }
      pos = data + size + (size % 2);
    }
    if (!channels || !frames || !bits || !sampleRate || !pcmStart || !pcmLen) throw new Error('unsupported AIFF structure');
    if (![8, 16, 24, 32].includes(bits)) throw new Error(`unsupported AIFF bit depth ${bits}`);

    const pcm = new Uint8Array(pcmLen);
    if (bits === 8) {
      pcm.set(bytes.subarray(pcmStart, pcmStart + pcmLen));
    } else {
      const step = bits / 8;
      for (let i = 0; i < pcmLen; i += step) {
        for (let b = 0; b < step; b++) pcm[i + b] = bytes[pcmStart + i + step - 1 - b];
      }
    }
    return wavBytes(pcm, channels, sampleRate, bits);
  }

  function readExtended80(view: DataView, offset: number): number {
    const expon = view.getUint16(offset, false);
    const hiMant = view.getUint32(offset + 2, false);
    const loMant = view.getUint32(offset + 6, false);
    if (expon === 0 && hiMant === 0 && loMant === 0) return 0;
    const sign = expon & 0x8000 ? -1 : 1;
    const exp = (expon & 0x7fff) - 16383;
    const mant = hiMant * 2 ** -31 + loMant * 2 ** -63;
    return Math.round(sign * mant * 2 ** exp);
  }

  function wavBytes(pcm: Uint8Array, channels: number, sampleRate: number, bits: number): Uint8Array {
    const out = new Uint8Array(44 + pcm.length);
    const v = new DataView(out.buffer);
    const put = (o: number, s: string) => { for (let i = 0; i < s.length; i++) out[o + i] = s.charCodeAt(i); };
    put(0, 'RIFF'); v.setUint32(4, 36 + pcm.length, true); put(8, 'WAVE'); put(12, 'fmt ');
    v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, channels, true);
    v.setUint32(24, sampleRate, true); v.setUint32(28, sampleRate * channels * bits / 8, true);
    v.setUint16(32, channels * bits / 8, true); v.setUint16(34, bits, true); put(36, 'data');
    v.setUint32(40, pcm.length, true); out.set(pcm, 44); return out;
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
    // On Android, audio formats like WMA aren't supported by <audio>.
    // Launch native player directly for audio — it supports more formats.
    if (media_kind === 'audio') {
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
    if (await tryAiffWavUrl()) return;

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

    // On Android/Tauri: let the user choose built-in/native/plugin/external for video and audio.
    if (isAndroidTauri && (media_kind === 'video' || media_kind === 'audio')) {
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
