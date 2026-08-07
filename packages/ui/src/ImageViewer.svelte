<script lang="ts">
  import { onMount } from "svelte";
  import { assetUrlForPath } from "@viewit/platform";

  let {
    uri,
    format,
    name,
    byte_len,
    asset_path = "",
    stream_url = "",
  }: {
    uri: string;
    format:
      | "image-png"
      | "image-jpg"
      | "image-webp"
      | "image-gif"
      | "image-bmp"
      | "image-tiff"
      | "image-svg"
      | "image-heic"
      | "image-psd"
      | "image-raw";
    name: string;
    byte_len: number;
    asset_path?: string;
    stream_url?: string;
  } = $props();

  let isSvg = $derived(format === "image-svg");
  let isHeic = $derived(format === "image-heic");
  let src = $state("");
  let loading = $state(true);
  let errorMsg = $state("");

  onMount(async () => {
    try {
      if (isHeic) {
        // Try pure-Rust HEIC decoder first (handles .heic/.heif).
        // For AVIF (.avif) this will fail — fall through to <img> below.
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const dataUrl = await invoke<string>("decode_heic_to_data_url", {
            assetPath: asset_path || "",
            uri: uri || "",
          });
          src = dataUrl;
        } catch {
          // AVIF or other image-heic that the Rust decoder can't handle.
          // Fall through to regular <img> path.
          if (stream_url) {
            src = stream_url;
          } else {
            src = await assetUrlForPath(uri, asset_path || undefined);
          }
        }
      } else if (stream_url) {
        src = stream_url;
      } else {
        src = await assetUrlForPath(uri, asset_path || undefined);
      }
    } catch (e) {
      errorMsg = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  });
</script>

<article class="image-viewer">
  <aside class="meta">
    <strong>{name}</strong>
    <span class="format">{format}</span>
    <span class="size">{byte_len.toLocaleString()} bytes</span>
  </aside>
  <div class="frame">
    {#if loading}
      <p class="status">Loading image…</p>
    {:else if errorMsg}
      <p class="status error">{errorMsg}</p>
    {:else if isSvg}
      <object data={src} type="image/svg+xml" class="svg-embed" title={name}>
        <img {src} alt={name} />
      </object>
    {:else}
      <img {src} alt={name} loading="lazy" />
    {/if}
  </div>
</article>

<style>
  .image-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .meta {
    display: flex;
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
  .format {
    background: var(--bg-secondary);
    padding: 0.1rem 0.4rem;
    border-radius: 0.3rem;
    font-family: ui-monospace, monospace;
  }
  .status {
    color: var(--text-secondary);
    font-style: italic;
    padding: 1rem;
  }
  .status.error {
    color: var(--error, #e74c3c);
  }
  .frame {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    background: var(--bg-secondary);
    overflow: auto;
  }
  .frame img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .svg-embed {
    width: 100%;
    height: 100%;
  }
</style>
