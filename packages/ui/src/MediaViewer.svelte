<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { debugLog, openWithExternal } from "@viewit/platform";
  import RuntimeChooser from "./RuntimeChooser.svelte";
  import type { PluginInfo } from "./pluginBridge";

  let {
    uri,
    asset_path = "",
    media_kind = "video",
    name = "media",
    format = "video",
    ext = "mp4",
    stream_url = "",
  }: {
    uri: string;
    asset_path?: string;
    media_kind?: "video" | "audio";
    name?: string;
    format?: string;
    ext?: string;
    stream_url?: string;
  } = $props();

  let src = $state("");
  let errorMsg = $state("");
  let showExternalBtn = $state(false);
  let mediaEl = $state<HTMLVideoElement | HTMLAudioElement | null>(null);
  let loadStart = 0;
  let blobUrl = $state("");
  let currentStrategy = $state("");
  let nativePlayerLaunched = $state(false);
  let isAndroidTauri = $state(false);
  let runtimeChooserOpen = $state(false);
  let playing = $state(false);
  let currentTime = $state(0);
  let duration = $state(0);
  let volume = $state(1);
  let playbackRate = $state(1);

  const MIME_MAP: Record<string, string> = {
    mp4: "video/mp4",
    m4v: "video/mp4",
    webm: "video/webm",
    mkv: "video/x-matroska",
    mov: "video/quicktime",
    "3gp": "video/3gpp",
    avi: "video/x-msvideo",
    mpg: "video/mpeg",
    mpeg: "video/mpeg",
    wmv: "video/x-ms-wmv",
    flv: "video/x-flv",
    ts: "video/mp2t",
    m2ts: "video/mp2t",
    mts: "video/mp2t",
    ogv: "video/ogg",
    f4v: "video/mp4",
    asf: "video/x-ms-wmv",
    mp3: "audio/mpeg",
    m4a: "audio/mp4",
    aac: "audio/aac",
    flac: "audio/flac",
    ogg: "audio/ogg",
    wav: "audio/wav",
    wma: "audio/x-ms-wma",
    opus: "audio/opus",
    aif: "audio/aiff",
    aiff: "audio/aiff",
  };

  function mimeForExt(e: string): string {
    return MIME_MAP[e.toLowerCase()] ?? (media_kind === "video" ? "video/mp4" : "audio/mpeg");
  }

  async function tryStreamProtocol(): Promise<boolean> {
    const { debugLog: log } = await import("@viewit/platform");
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const streamUrl = await invoke<string>("media_stream_url", {
        assetPath: asset_path,
        ext: ext || "mp4",
      });
      log(`[media] stream URL: ${streamUrl}`);
      if (streamUrl) {
        src = streamUrl;
        currentStrategy = "stream";
        return true;
      }
    } catch (e) {
      log(`[media] stream protocol failed: ${e instanceof Error ? e.message : String(e)}`);
    }
    return false;
  }

  async function tryConvertFileSrc(): Promise<boolean> {
    const { debugLog: log } = await import("@viewit/platform");
    try {
      const { convertFileSrc } = await import("@tauri-apps/api/core");
      const filePath = asset_path.startsWith("file://") ? asset_path.slice(7) : asset_path;
      const converted = convertFileSrc(filePath);
      log(`[media] convertFileSrc → ${converted?.slice(0, 120)}`);
      if (converted) {
        src = converted;
        currentStrategy = "convertFileSrc";
        return true;
      }
    } catch (e) {
      log(`[media] convertFileSrc failed: ${e instanceof Error ? e.message : String(e)}`);
    }
    return false;
  }

  function bytesArrayBuffer(bytes: Uint8Array): ArrayBuffer {
    return Uint8Array.from(bytes).buffer;
  }

  async function tryBlobUrl(): Promise<boolean> {
    const { debugLog: log } = await import("@viewit/platform");
    try {
      log(`[media] reading bytes via IPC…`);
      const { readMaterializedBytes } = await import("@viewit/platform");
      const uint8 = await readMaterializedBytes(asset_path);
      log(`[media] got ${uint8.length} bytes`);
      const mime = mimeForExt(ext);
      const blob = new Blob([bytesArrayBuffer(uint8)], { type: mime });
      blobUrl = URL.createObjectURL(blob);
      src = blobUrl;
      currentStrategy = "blob";
      log(`[media] blob URL created, size=${blob.size}`);
      return true;
    } catch (e) {
      log(`[media] blob fallback failed: ${e instanceof Error ? e.message : String(e)}`);
    }
    return false;
  }

  async function trySpecialAudioToWavUrl(): Promise<boolean> {
    if (ext !== "aif" && ext !== "aiff" && ext !== "caf") return false;
    const { debugLog: log } = await import("@viewit/platform");
    try {
      const { readUriBytes, readMaterializedBytes } = await import("@viewit/platform");
      const bytes = await (asset_path ? readMaterializedBytes(asset_path) : readUriBytes(uri));
      if (ext === "aif" || ext === "aiff") {
        const wav = aiffToWav(bytes);
        blobUrl = URL.createObjectURL(new Blob([bytesArrayBuffer(wav)], { type: "audio/wav" }));
        src = blobUrl;
        currentStrategy = "aiff-wav";
        log(`[media] AIFF decoded to WAV bytes=${wav.length}`);
        return true;
      }
      if (ext === "caf") {
        const wav = cafToWav(bytes);
        blobUrl = URL.createObjectURL(new Blob([bytesArrayBuffer(wav)], { type: "audio/wav" }));
        src = blobUrl;
        currentStrategy = "caf-wav";
        log(`[media] CAF decoded to WAV bytes=${wav.length}`);
        return true;
      }
      return false;
    } catch (e) {
      log(`[media] audio decode failed: ${e instanceof Error ? e.message : String(e)}`);
      return false;
    }
  }

  function cafToWav(bytes: Uint8Array): Uint8Array {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const ascii = (offset: number, len: number) =>
      String.fromCharCode(...bytes.subarray(offset, offset + len));
    if (ascii(0, 4) !== "caff") throw new Error("not Apple Core Audio Format (caff)");

    let sampleRate = 44100;
    let formatID = "";
    let formatFlags = 0;
    let bytesPerPacket = 0;
    let framesPerPacket = 0;
    let channelsPerFrame = 0;
    let bitsPerChannel = 0;
    let pcmStart = 0;
    let pcmLen = 0;

    for (let pos = 8; pos + 12 <= bytes.length;) {
      const chunkType = ascii(pos, 4);
      // CAF chunk size is 64-bit int
      const chunkSizeHi = view.getUint32(pos + 4, false);
      const chunkSizeLo = view.getUint32(pos + 8, false);
      const chunkSize =
        chunkSizeHi === 0
          ? chunkSizeLo
          : Number((BigInt(chunkSizeHi) << 32n) | BigInt(chunkSizeLo));
      const dataPos = pos + 12;

      if (chunkType === "desc") {
        sampleRate = view.getFloat64(dataPos, false);
        formatID = ascii(dataPos + 8, 4);
        formatFlags = view.getUint32(dataPos + 12, false);
        bytesPerPacket = view.getUint32(dataPos + 16, false);
        framesPerPacket = view.getUint32(dataPos + 20, false);
        channelsPerFrame = view.getUint32(dataPos + 24, false);
        bitsPerChannel = view.getUint32(dataPos + 28, false);
      } else if (chunkType === "data") {
        // data chunk has 4 bytes editCount followed by data
        pcmStart = dataPos + 4;
        pcmLen =
          chunkSize < 0 || chunkSize === 0xffffffff ? bytes.length - pcmStart : chunkSize - 4;
      }

      if (chunkSize < 0 || chunkSize === 0xffffffff) break;
      pos = dataPos + chunkSize;
    }

    if (!pcmStart || pcmLen <= 0) throw new Error("missing or invalid CAF audio data chunk");

    // Handle Linear PCM ('lpcm')
    if (formatID === "lpcm") {
      const isFloat = (formatFlags & (1 << 0)) !== 0;
      const isLittleEndian = (formatFlags & (1 << 1)) !== 0;
      if (isFloat) throw new Error("floating-point LPCM in CAF not supported directly");

      const bits = bitsPerChannel || 16;
      const channels = channelsPerFrame || 1;
      const pcm = new Uint8Array(pcmLen);

      if (isLittleEndian || bits === 8) {
        pcm.set(bytes.subarray(pcmStart, pcmStart + pcmLen));
      } else {
        // Swap big-endian to little-endian for WAV
        const step = bits / 8;
        for (let i = 0; i < pcmLen; i += step) {
          for (let b = 0; b < step; b++) {
            pcm[i + b] = bytes[pcmStart + i + step - 1 - b];
          }
        }
      }
      return wavBytes(pcm, channels, Math.round(sampleRate), bits);
    }

    throw new Error(`unsupported compressed CAF format '${formatID}' (requires LPCM)`);
  }

  function aiffToWav(bytes: Uint8Array): Uint8Array {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const ascii = (offset: number, len: number) =>
      String.fromCharCode(...bytes.subarray(offset, offset + len));
    if (ascii(0, 4) !== "FORM" || ascii(8, 4) !== "AIFF") throw new Error("not AIFF");
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
      if (id === "COMM") {
        channels = view.getUint16(data, false);
        frames = view.getUint32(data + 2, false);
        bits = view.getUint16(data + 6, false);
        sampleRate = readExtended80(view, data + 8);
      } else if (id === "SSND") {
        const offset = view.getUint32(data, false);
        pcmStart = data + 8 + offset;
        pcmLen = size - 8 - offset;
      }
      pos = data + size + (size % 2);
    }
    if (!channels || !frames || !bits || !sampleRate || !pcmStart || !pcmLen)
      throw new Error("unsupported AIFF structure");
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

  function wavBytes(
    pcm: Uint8Array,
    channels: number,
    sampleRate: number,
    bits: number,
  ): Uint8Array {
    const out = new Uint8Array(44 + pcm.length);
    const v = new DataView(out.buffer);
    const put = (o: number, s: string) => {
      for (let i = 0; i < s.length; i++) out[o + i] = s.charCodeAt(i);
    };
    put(0, "RIFF");
    v.setUint32(4, 36 + pcm.length, true);
    put(8, "WAVE");
    put(12, "fmt ");
    v.setUint32(16, 16, true);
    v.setUint16(20, 1, true);
    v.setUint16(22, channels, true);
    v.setUint32(24, sampleRate, true);
    v.setUint32(28, (sampleRate * channels * bits) / 8, true);
    v.setUint16(32, (channels * bits) / 8, true);
    v.setUint16(34, bits, true);
    put(36, "data");
    v.setUint32(40, pcm.length, true);
    out.set(pcm, 44);
    return out;
  }

  async function launchNativePlayer() {
    const { debugLog: log } = await import("@viewit/platform");
    try {
      const videoUri = stream_url || uri;
      log(`[media] launching native player: ${videoUri?.slice(0, 80)}`);
      if ("AndroidBridge" in window) {
        (window as any).AndroidBridge.launchVideoPlayer(videoUri, name, ext || "");
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
    if (media_kind === "audio") {
      await launchNativePlayer();
      return;
    }
    await useWebRuntime();
  }

  async function useInstalledPlugin(_plugin: PluginInfo) {
    await launchNativePlayer();
  }

  async function useWebRuntime() {
    const { debugLog: log } = await import("@viewit/platform");
    // Web (no Tauri): the frontend already hands us a loadable `blob:`/`data:`
    // URL via the picker. Use it directly — no Tauri IPC needed.
    if (/^(blob:|data:|https?:)/.test(uri)) {
      src = uri;
      currentStrategy = "direct-url";
      log(`[media] using direct URL (web) ${uri.slice(0, 60)}…`);
      return;
    }

    if (await trySpecialAudioToWavUrl()) return;

    if (stream_url) {
      log(`[media] using direct stream_url`);
      src = stream_url;
      currentStrategy = "stream_url";
      return;
    }

    if (!asset_path) {
      errorMsg = "No asset path — materialization may have failed";
      return;
    }

    const IS_TAURI = "__TAURI_INTERNALS__" in window;
    if (!IS_TAURI) {
      await tryBlobUrl();
      return;
    }

    if (!(await tryStreamProtocol())) {
      if (!(await tryConvertFileSrc())) {
        await tryBlobUrl();
      }
    }
    log(`[media] src set via ${currentStrategy}, waiting for load…`);
  }

  onMount(async () => {
    const { debugLog: log } = await import("@viewit/platform");

    isAndroidTauri = "__TAURI_INTERNALS__" in window;

    log(`[media] uri=${uri?.slice(0, 80)}`);
    log(`[media] asset_path=${asset_path?.slice(0, 80)}`);
    log(`[media] stream_url=${stream_url?.slice(0, 80)}`);
    log(`[media] kind=${media_kind} format=${format} ext=${ext}`);

    loadStart = Date.now();

    // On Android/Tauri: video and non-web-playable audio (e.g. WMA) go through the
    // runtime chooser. Formats the WebView <audio> can decode render in-app.
    const webPlayableAudio = new Set([
      "mp3",
      "wav",
      "flac",
      "ogg",
      "opus",
      "m4a",
      "aac",
      "aif",
      "aiff",
    ]);
    if (
      isAndroidTauri &&
      (media_kind === "video" ||
        (media_kind === "audio" && !webPlayableAudio.has(ext.toLowerCase())))
    ) {
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
    syncPlaybackState();
  }

  function syncPlaybackState() {
    if (!mediaEl) return;
    currentTime = Number.isFinite(mediaEl.currentTime) ? mediaEl.currentTime : 0;
    duration = Number.isFinite(mediaEl.duration) ? mediaEl.duration : 0;
    playing = !mediaEl.paused && !mediaEl.ended;
  }

  async function togglePlayback() {
    if (!mediaEl) return;
    if (mediaEl.paused) await mediaEl.play();
    else mediaEl.pause();
    syncPlaybackState();
  }

  function seek(event: Event) {
    if (!mediaEl) return;
    const value = Number((event.currentTarget as HTMLInputElement).value);
    mediaEl.currentTime = value;
    currentTime = value;
  }

  function setVolume(event: Event) {
    if (!mediaEl) return;
    volume = Number((event.currentTarget as HTMLInputElement).value);
    mediaEl.volume = volume;
  }

  function setPlaybackRate(event: Event) {
    if (!mediaEl) return;
    playbackRate = Number((event.currentTarget as HTMLSelectElement).value);
    mediaEl.playbackRate = playbackRate;
  }

  function formatTime(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
    const whole = Math.floor(seconds);
    const hours = Math.floor(whole / 3600);
    const minutes = Math.floor((whole % 3600) / 60);
    const secs = String(whole % 60).padStart(2, "0");
    return hours > 0
      ? `${hours}:${String(minutes).padStart(2, "0")}:${secs}`
      : `${minutes}:${secs}`;
  }

  async function onError() {
    const el = mediaEl;
    if (!el) return;
    const err = (el as HTMLVideoElement).error;
    const code = err?.code ?? 0;
    const NAMES: Record<number, string> = {
      1: "ABORTED",
      2: "NETWORK",
      3: "DECODE",
      4: "SRC_NOT_SUPPORTED",
    };
    const msg = err?.message || NAMES[code] || `code ${code}`;
    debugLog(
      `[media] ERROR code=${code} (${NAMES[code] ?? "?"}): ${msg} strategy=${currentStrategy}`,
    );

    src = "";

    if (code === 4) {
      if (
        currentStrategy === "stream_url" ||
        currentStrategy === "stream" ||
        currentStrategy === "convertFileSrc"
      ) {
        debugLog(`[media] codec unsupported (${ext}), offering external player`);
        errorMsg = `Format .${ext} isn't supported by the built-in player.`;
        showExternalBtn = true;
        return;
      }
    }

    if (currentStrategy === "convertFileSrc" && code === 4) {
      debugLog(`[media] convertFileSrc failed, trying blob…`);
      if (await tryBlobUrl()) {
        loadStart = Date.now();
        return;
      }
    }

    if (currentStrategy === "stream" && code === 4) {
      debugLog(`[media] stream failed, trying convertFileSrc…`);
      if (await tryConvertFileSrc()) {
        loadStart = Date.now();
        return;
      }
      debugLog(`[media] convertFileSrc failed, trying blob…`);
      if (await tryBlobUrl()) {
        loadStart = Date.now();
        return;
      }
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
  function onWaiting() {
    debugLog(`[media] WAITING`);
  }
  function onEnded() {
    debugLog(`[media] ENDED`);
    syncPlaybackState();
  }
</script>

<article class="media-viewer">
  <aside class="meta">
    <strong>{name}</strong>
    <span class="tag">{format}</span>
    {#if currentStrategy}<span class="hint">strategy: {currentStrategy}</span>{/if}
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
    {:else if media_kind === "audio"}
      <audio
        bind:this={mediaEl}
        {src}
        onerror={onError}
        onloadedmetadata={onLoad}
        ontimeupdate={syncPlaybackState}
        onplay={syncPlaybackState}
        onpause={syncPlaybackState}
        onended={syncPlaybackState}
      ></audio>
    {:else}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        bind:this={mediaEl}
        playsinline
        preload="auto"
        {src}
        onerror={onError}
        onloadedmetadata={onLoad}
        ontimeupdate={syncPlaybackState}
        onplay={syncPlaybackState}
        onpause={syncPlaybackState}
        onprogress={onProgress}
        onwaiting={onWaiting}
        onended={onEnded}
      ></video>
    {/if}
  </div>
  {#if src && !errorMsg}
    <div class="controls" aria-label="Media controls">
      <button class="play-button" onclick={togglePlayback} aria-label={playing ? "Pause" : "Play"}>
        {playing ? "Pause" : "Play"}
      </button>
      <span class="time">{formatTime(currentTime)}</span>
      <input
        class="seek"
        type="range"
        min="0"
        max={duration || 0}
        step="0.01"
        value={currentTime}
        oninput={seek}
        aria-label="Seek"
        disabled={!duration}
      />
      <span class="time">{formatTime(duration)}</span>
      <label class="volume-control">
        <span>Volume</span>
        <input type="range" min="0" max="1" step="0.05" value={volume} oninput={setVolume} />
      </label>
      <label class="speed-control">
        <span>Speed</span>
        <select value={playbackRate} onchange={setPlaybackRate}>
          <option value="0.5">0.5x</option>
          <option value="0.75">0.75x</option>
          <option value="1">1x</option>
          <option value="1.25">1.25x</option>
          <option value="1.5">1.5x</option>
          <option value="2">2x</option>
        </select>
      </label>
    </div>
  {/if}
</article>

<RuntimeChooser
  open={runtimeChooserOpen}
  uri={stream_url || uri}
  {name}
  {ext}
  builtInLabel="Native Android player"
  service="viewit.media.play"
  onClose={() => (runtimeChooserOpen = false)}
  onUseBuiltIn={useBuiltInRuntime}
  onUseInstalledPlugin={useInstalledPlugin}
/>

<style>
  .media-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 40vh;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: baseline;
    padding: 0.5rem 1rem;
    border-bottom: 1px solid var(--border);
    font-size: 0.8rem;
    color: var(--text-secondary);
  }
  .meta strong {
    color: var(--text-primary);
  }
  .tag {
    font-family: ui-monospace, monospace;
    background: var(--bg-secondary);
    padding: 0.1rem 0.4rem;
    border-radius: 0.25rem;
  }
  .hint {
    font-size: 0.7rem;
    opacity: 0.85;
  }
  .frame {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    background: #000;
  }
  video {
    max-width: 100%;
    max-height: 70vh;
  }
  audio {
    display: none;
  }
  .controls {
    display: grid;
    grid-template-columns: auto auto minmax(6rem, 1fr) auto auto auto;
    align-items: center;
    gap: 0.65rem;
    padding: 0.7rem 1rem;
    border-top: 1px solid var(--border);
    background: var(--bg-secondary);
    color: var(--text-primary);
  }
  .play-button,
  .speed-control select {
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    padding: 0.35rem 0.65rem;
    color: var(--text-primary);
    background: var(--bg-primary, #161b22);
    cursor: pointer;
  }
  .seek,
  .volume-control input {
    accent-color: var(--link);
  }
  .seek {
    width: 100%;
  }
  .time {
    min-width: 2.8rem;
    color: var(--text-secondary);
    font:
      0.75rem ui-monospace,
      monospace;
    text-align: center;
  }
  .volume-control,
  .speed-control {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    color: var(--text-secondary);
    font-size: 0.75rem;
  }
  .volume-control input {
    width: 5rem;
  }
  .status {
    color: var(--text-secondary);
    font-style: italic;
  }
  .err {
    color: var(--error);
    white-space: pre-wrap;
    text-align: center;
    padding: 1rem;
    font-size: 0.85rem;
  }
  .external-btn {
    margin-top: 0.75rem;
    padding: 0.5rem 1.2rem;
    background: var(--link);
    color: #fff;
    border: none;
    border-radius: 0.4rem;
    font-size: 0.85rem;
    cursor: pointer;
    font-weight: 500;
  }
  .external-btn:hover {
    opacity: 0.9;
  }
  @media (max-width: 700px) {
    .controls {
      grid-template-columns: auto auto minmax(4rem, 1fr) auto;
      gap: 0.4rem;
      padding: 0.6rem;
    }
    .volume-control {
      grid-column: 1 / 4;
    }
    .volume-control input {
      width: 100%;
    }
    .speed-control {
      grid-column: 4;
      grid-row: 2;
    }
  }
</style>
