<script lang="ts">
  // Phase 2.1 — images native pass-through.
  // Per plan §5: webview's <img>/<svg> decode the formats; we just hand them
  // the file bytes via convertFileSrc on Tauri hosts, or via the object URL
  // on web. Zero bundled decoder.

  import { onMount } from 'svelte';
  import { assetUrlForPath } from '@viewit/platform';

  let {
    uri,
    format,
    name,
    byte_len,
    asset_path = '',
  }: {
    uri: string;
    format: 'image-png' | 'image-jpg' | 'image-webp' | 'image-gif'
          | 'image-bmp' | 'image-tiff' | 'image-svg' | 'image-heic' | 'image-psd';
    name: string;
    byte_len: number;
    asset_path?: string;
  } = $props();

  const isSvg = format === 'image-svg';
  let src = $state('');
  onMount(async () => {
    src = await assetUrlForPath(uri, asset_path || undefined);
  });

  // `<img>` happily embeds SVGs too, so we don't actually need a separate
  // code path. We keep the plumbing uniform.

  // On Tauri the URI is file:// (or content:// on Android); on web it's a
  // blob: URL the platform layer created. Either way, browsers know how to
  // retrieve it as bytes for `<img>`.
</script>

<article class="image-viewer">
  <aside class="meta">
    <strong>{name}</strong>
    <span class="format">{format}</span>
    <span class="size">{byte_len.toLocaleString()} bytes</span>
  </aside>
  <div class="frame">
    {#if !src}
      <p class="status">Loading image…</p>
    {:else if isSvg}
      <!-- Inline-fetch SVG so we get native styling + pointer events -->
      <object data={src} type="image/svg+xml" class="svg-embed" title={name}>
        <img src={src} alt={name} />
      </object>
    {:else}
      <img src={src} alt={name} loading="lazy" />
    {/if}
  </div>
</article>

<style>
  .image-viewer { display: flex; flex-direction: column; height: 100%; }
  .meta { display: flex; gap: 0.5rem; align-items: baseline; padding: 0.5rem 1rem; border-bottom: 1px solid var(--border); font-size: 0.8rem; color: var(--text-secondary); }
  .meta strong { color: var(--text-primary); }
  .format { background: var(--bg-secondary); padding: 0.1rem 0.4rem; border-radius: 0.3rem; font-family: ui-monospace, monospace; }
  .status { color: var(--text-secondary); font-style: italic; padding: 1rem; }
  .frame { flex: 1; display: flex; align-items: center; justify-content: center; padding: 1rem; background: var(--bg-secondary); overflow: auto; }
  .frame img { max-width: 100%; max-height: 100%; object-fit: contain; }
  .svg-embed { width: 100%; height: 100%; }
</style>
