<script lang="ts">
  // Top-level dispatcher: given a Document variant, render the right viewer.
  // Per plan §2: "Svelte viewer components" consume "Structured JSON over IPC".
  //
  // `uri` is carried alongside the Document so Image (Phase 2.1) and Unsupported
  // (Phase 4 — "Open with…" hand-off per ADR 0004) viewers can act on it.

  import type { Document } from '@viewit/platform';
  import TextViewer from './TextViewer.svelte';
  import ImageViewer from './ImageViewer.svelte';
  import MarkdownViewer from './MarkdownViewer.svelte';
  import JsonViewer from './JsonViewer.svelte';
  import CsvViewer from './CsvViewer.svelte';
  import UnsupportedViewer from './UnsupportedViewer.svelte';
  import PlaceholderViewer from './PlaceholderViewer.svelte';

  let {
    root,
    initialFile = undefined
  }: {
    root: 'desktop' | 'mobile' | 'web';
    initialFile?: string;
  } = $props();

  let doc: Document | null = $state(null);
  let docUri: string | null = initialFile ?? null;
  let pendingUri: string | null = initialFile ?? null;
  let busy = $state(false);
  let error: string | null = $state(null);

  import { onMount } from 'svelte';
  import { openFile, openedFiles, onOpenedFiles } from '@viewit/platform';
  onMount(async () => {
    try {
      const cold = await openedFiles();
      if (cold.length > 0) {
        pendingUri = cold[0];
        await load();
      }
    } catch (e) {
      console.debug('openedFiles unavailable in this env:', e);
    }
    onOpenedFiles((urls) => {
      if (urls.length > 0) {
        pendingUri = urls[0];
        void load();
      }
    });
  });

  async function load() {
    if (!pendingUri) return;
    busy = true;
    error = null;
    docUri = pendingUri;
    try {
      doc = await openFile(pendingUri);
    } catch (e: any) {
      error = e?.toString?.() ?? String(e);
    } finally {
      busy = false;
    }
  }

  async function pick() {
    const input = document.createElement('input');
    input.type = 'file';
    input.onchange = async () => {
      const f = input.files?.[0];
      if (!f) return;
      const url = URL.createObjectURL(f);
      pendingUri = url;
      await load();
      URL.revokeObjectURL(url);
    };
    input.click();
  }
</script>

<div class="viewit-root" data-root={root}>
  <header>
    <h1>ViewIt</h1>
    <button onclick={pick}>Open file…</button>
  </header>

  <main>
    {#if busy}
      <p class="status">Reading file…</p>
    {:else if error}
      <pre class="error">{error}</pre>
    {:else if doc}
      {#if doc.kind === 'text'}
        <TextViewer {...(doc as any)} />
      {:else if doc.kind === 'image' && docUri}
        <ImageViewer uri={docUri} {...(doc as any)} />
      {:else if doc.kind === 'markdown'}
        <MarkdownViewer {...(doc as any)} />
      {:else if doc.kind === 'json'}
        <JsonViewer {...(doc as any)} />
      {:else if doc.kind === 'csv'}
        <CsvViewer {...(doc as any)} />
      {:else if doc.kind === 'unsupported'}
        <UnsupportedViewer uri={docUri ?? undefined} {...(doc as any)} />
      {:else if doc.kind === 'placeholder'}
        <PlaceholderViewer {...(doc as any)} />
      {:else}
        <p class="error">Unknown document kind: <code>{(doc as any).kind}</code></p>
      {/if}
    {:else}
      <p class="empty">Drop a file or pick one — everything opens.</p>
    {/if}
  </main>
</div>

<style>
  :global(*) { box-sizing: border-box; }
  :global(body) { margin: 0; font-family: system-ui, sans-serif; color: #222; background: #fff; }
  .viewit-root { display: flex; flex-direction: column; min-height: 100vh; }
  header { display: flex; align-items: center; justify-content: space-between; padding: 0.5rem 1rem; border-bottom: 1px solid #ddd; }
  header h1 { font-size: 1.2rem; margin: 0; letter-spacing: -0.01em; }
  header button { cursor: pointer; }
  main { padding: 1rem; flex: 1; }
  .status, .empty { color: #888; font-style: italic; }
  .error { color: #b22; white-space: pre-wrap; }
</style>
