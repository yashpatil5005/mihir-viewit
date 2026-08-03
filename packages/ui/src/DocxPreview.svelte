<script lang="ts">
  // Real DOCX rendering via docx-preview (docxjs) — paginated layout, fonts,
  // images, tables, headers/footers, footnotes. Runs in the WebView DOM, so no
  // native plugin or WASM is required.
  import { onMount } from 'svelte';
  import { hasAndroidBridge, materializeExternalUri } from './pluginBridge';

  let {
    source_uri = '',
  }: { source_uri?: string } = $props();

  let hostEl: HTMLDivElement | null = $state(null);
  let status = $state<'loading' | 'ready' | 'error'>('loading');
  let errorMsg = $state('');
  let loadedModule: any = null;

  function isTauri(): boolean {
    return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in (window ?? {});
  }

  async function readBytes(uri: string): Promise<Uint8Array> {
    if (isTauri()) {
      const { readMaterializedBytes } = await import('@viewit/platform');
      return readMaterializedBytes(uri);
    }
    const res = await fetch(uri);
    if (!res.ok) throw new Error(`Failed to read ${uri}: ${res.status} ${res.statusText}`);
    return new Uint8Array(await res.arrayBuffer());
  }

  async function renderDoc() {
    status = 'loading';
    errorMsg = '';
    if (!source_uri) {
      status = 'error';
      errorMsg = 'No source URI provided';
      return;
    }
    try {
      let readable = source_uri;
      if (hasAndroidBridge()) {
        readable = materializeExternalUri(source_uri, 'docx');
      }
      const bytes = await readBytes(readable);
      if (!loadedModule) {
        loadedModule = await import('docx-preview');
      }
      if (!hostEl) {
        status = 'error';
        errorMsg = 'Container not ready';
        return;
      }
      hostEl.innerHTML = '';
      await loadedModule.renderAsync(bytes, hostEl, null, {
        className: 'docx',
        inWrapper: true,
        breakPages: false,
        ignoreLastRenderedPageBreak: false,
        useBase64URL: true,
        experimental: true,
      });
      status = 'ready';
    } catch (e) {
      status = 'error';
      errorMsg = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(() => {
    void renderDoc();
  });
</script>

<div class="docx-preview-viewer">
  {#if status === 'loading'}
    <p class="status">Rendering document…</p>
  {:else if status === 'error'}
    <p class="error">Unable to render this Word document: {errorMsg}</p>
  {/if}
  <div class="docx-surface" class:busy={status !== 'ready'} bind:this={hostEl}></div>
</div>

<style>
  .docx-preview-viewer {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    min-height: 40vh;
  }
  .status { color: var(--text-secondary); }
  .error { color: var(--danger, #e5484d); }
  .docx-surface {
    overflow: auto;
    border-radius: 0.8rem;
  }
  .docx-surface.busy { visibility: hidden; }
  .docx-surface :global(.docx-wrapper) {
    background: #fff;
    border-radius: 0.6rem;
  }
</style>
