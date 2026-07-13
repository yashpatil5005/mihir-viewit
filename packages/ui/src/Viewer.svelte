<script lang="ts">
  // Top-level dispatcher: given a Document variant, render the right viewer.
  // Per plan §2: "Svelte viewer components" consume "Structured JSON over IPC".
  //
  // `uri` is carried alongside the Document so Image (Phase 2.1) and Unsupported
  // (Phase 4 — "Open with…" hand-off per ADR 0004) viewers can act on it.

  import type { Document } from '@viewit/platform';
  import TextViewer from './TextViewer.svelte';
  import ImageViewer from './ImageViewer.svelte';
  import UnsupportedViewer from './UnsupportedViewer.svelte';
  import PlaceholderViewer from './PlaceholderViewer.svelte';
  import GridView from './GridView.svelte';
  import Onboarding from './Onboarding.svelte';

  // Phase 3.8 — per-format lazy code-split. Heavy viewers load on demand via
  // dynamic import() so opening a .txt never pulls in the PDF/Office chunks.
  type Comp = { default: any };
  let MarkdownViewer = $state<any>(null);
  let JsonViewer = $state<any>(null);
  let CsvViewer = $state<any>(null);
  let PdfViewer = $state<any>(null);
  let EpubViewer = $state<any>(null);
  let ArchiveViewer = $state<any>(null);
  let PptxViewer = $state<any>(null);
  let DocxViewer = $state<any>(null);
  let XlsxViewer = $state<any>(null);
  let IcsViewer = $state<any>(null);
  let VcfViewer = $state<any>(null);
  let DesktopEntryViewer = $state<any>(null);

  // Pre-resolve the lazy chunk for the current document kind once we know it.
  $effect(() => {
    if (!doc) return;
    const k = (doc as any).kind;
    const uri = docUri ?? '';
    const isIcs = /\.ics?$/i.test(uri);
    const isVcf = /\.vcf$/i.test(uri);
    const isDesktop = /\.desktop$/i.test(uri);
    const loaders: Record<string, (() => Promise<Comp>) | undefined> = {
      'markdown': () => import('./MarkdownViewer.svelte'),
      'json':     () => import('./JsonViewer.svelte'),
      'csv':      () => import('./CsvViewer.svelte'),
      'pdf':      () => import('./PdfViewer.svelte'),
      'epub':     () => import('./EpubViewer.svelte'),
      'archive':  () => import('./ArchiveViewer.svelte'),
      'pptx':     () => import('./PptxViewer.svelte'),
      'docx':     () => import('./DocxViewer.svelte'),
      'xlsx':     () => import('./XlsxViewer.svelte'),
    };
    let loader = loaders[k] as (() => Promise<Comp>) | undefined;
    if (k === 'text') {
      if (isIcs) loader = () => import('./IcsViewer.svelte');
      else if (isVcf) loader = () => import('./VcfViewer.svelte');
      else if (isDesktop) loader = () => import('./DesktopEntryViewer.svelte');
      else loader = undefined;
    }
    if (!loader) return;
    loader().then((m) => {
      if (k === 'markdown') MarkdownViewer = m.default;
      else if (k === 'json') JsonViewer = m.default;
      else if (k === 'csv') CsvViewer = m.default;
      else if (k === 'pdf') PdfViewer = m.default;
      else if (k === 'epub') EpubViewer = m.default;
      else if (k === 'archive') ArchiveViewer = m.default;
      else if (k === 'pptx') PptxViewer = m.default;
      else if (k === 'docx') DocxViewer = m.default;
      else if (k === 'xlsx') XlsxViewer = m.default;
      else if (k === 'text' && isIcs) IcsViewer = m.default;
      else if (k === 'text' && isVcf) VcfViewer = m.default;
      else if (k === 'text' && isDesktop) DesktopEntryViewer = m.default;
    });
  });

  let mode: 'view' | 'browse' = $state('view');

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
  let busyHint = $state('');
  let error: string | null = $state(null);

  import { onMount } from 'svelte';
  import {
    openFile,
    openFileFromPicker,
    openedFiles,
    onOpenedFiles,
    checkFileBeforeRead,
  } from '@viewit/platform';

  let imagePreviewUrl: string | null = null;
  import { theme, toggleTheme, applyTheme } from './theme.svelte';
  onMount(async () => {
    applyTheme();
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
      if (f) await pickFile(f);
    };
    input.click();
  }

  async function pickFile(f: File) {
    if (imagePreviewUrl) {
      URL.revokeObjectURL(imagePreviewUrl);
      imagePreviewUrl = null;
    }
    busy = true;
    error = null;
    pendingUri = f.name;
    docUri = f.name;
    const mb = (f.size / 1_048_576).toFixed(1);
    try {
      const gate = checkFileBeforeRead(f);
      if (gate.reject) {
        doc = {
          kind: 'unsupported',
          format: 'unsupported',
          reason: gate.reason,
          suggestion: gate.openWithExternal ? 'open-with-external' : 'none',
        };
        return;
      }
      busyHint = `Reading ${f.name} (${mb} MB)…`;
      doc = await openFileFromPicker(f);
      if (doc.kind === 'image') {
        imagePreviewUrl = URL.createObjectURL(f);
        docUri = imagePreviewUrl;
      }
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
      doc = null;
    } finally {
      busy = false;
      busyHint = '';
    }
  }
</script>

<div class="viewit-root" data-root={root}>
  <header>
    <h1>ViewIt</h1>
    <div class="header-actions">
      <button class="theme-toggle" onclick={toggleTheme} aria-label="Toggle dark mode" title="Toggle theme">
        {theme.mode === 'dark' ? '☀' : '☾'}
      </button>
      <button onclick={pick}>Open file…</button>
      <span class="hint" title="Videos and files over 32 MB are not loaded into memory">≤32 MB docs</span>
      <button class="mode-toggle" onclick={() => mode = mode === 'view' ? 'browse' : 'view'} aria-label="Toggle browse mode" title="Toggle browse mode">
        {mode === 'view' ? '▦' : '↩'}
      </button>
    </div>
  </header>

  <main>
    {#if mode === 'browse'}
      <GridView onPick={(f) => { mode = 'view'; pickFile(f); }} />
    {:else if !busy && !error && !doc}
      <Onboarding />
      <p class="empty">Drop a file or pick one — everything opens.</p>
      <GridView onPick={(f) => { mode = 'view'; pickFile(f); }} />
    {:else if busy}
      <p class="status">{busyHint || 'Reading file…'}</p>
    {:else if error}
      <pre class="error">{error}</pre>
    {:else if doc}
      {#if doc.kind === 'text'}
        {#if docUri && /\.ics?$/i.test(docUri) && IcsViewer}
          <IcsViewer {...(doc as any)} />
        {:else if docUri && /\.vcf$/i.test(docUri) && VcfViewer}
          <VcfViewer {...(doc as any)} />
        {:else if docUri && /\.desktop$/i.test(docUri) && DesktopEntryViewer}
          <DesktopEntryViewer {...(doc as any)} />
        {:else}
          <TextViewer {...(doc as any)} />
        {/if}
      {:else if doc.kind === 'image' && docUri}
        <ImageViewer uri={docUri} {...(doc as any)} />
      {:else if doc.kind === 'markdown' && MarkdownViewer}
        <MarkdownViewer {...(doc as any)} />
      {:else if doc.kind === 'json' && JsonViewer}
        <JsonViewer {...(doc as any)} />
      {:else if doc.kind === 'csv' && CsvViewer}
        <CsvViewer {...(doc as any)} />
      {:else if doc.kind === 'pdf' && PdfViewer}
        <PdfViewer {...(doc as any)} />
      {:else if doc.kind === 'epub' && EpubViewer}
        <EpubViewer {...(doc as any)} />
      {:else if doc.kind === 'archive' && ArchiveViewer}
        <ArchiveViewer {...(doc as any)} />
      {:else if doc.kind === 'pptx' && PptxViewer}
        <PptxViewer document={doc} />
      {:else if doc.kind === 'docx' && DocxViewer}
        <DocxViewer document={doc} />
      {:else if doc.kind === 'xlsx' && XlsxViewer}
        <XlsxViewer document={doc} />
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
  :global(:root) {
    --text-primary: #222;
    --text-secondary: #888;
    --bg-primary: #fff;
    --bg-secondary: #f7f7f7;
    --border: #ddd;
    --error: #b22;
    --link: #124FC2;
  }
  :global([data-theme="dark"]) {
    --text-primary: #e8e8e8;
    --text-secondary: #999;
    --bg-primary: #1a1a1a;
    --bg-secondary: #252525;
    --border: #3a3a3a;
    --error: #ff6b6b;
    --link: #6ea8ff;
  }
  :global(body) { margin: 0; font-family: system-ui, sans-serif; color: var(--text-primary); background: var(--bg-primary); }
  .viewit-root { display: flex; flex-direction: column; min-height: 100vh; }
  header { display: flex; align-items: center; justify-content: space-between; padding: 0.5rem 1rem; border-bottom: 1px solid var(--border); background: var(--bg-primary); }
  header h1 { font-size: 1.2rem; margin: 0; letter-spacing: -0.01em; color: var(--text-primary); }
  .header-actions { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .hint { font-size: 0.65rem; color: var(--text-secondary); opacity: 0.85; }
  header button { cursor: pointer; background: var(--bg-secondary); color: var(--text-primary); border: 1px solid var(--border); border-radius: 0.3rem; padding: 0.3rem 0.7rem; font-size: 0.85rem; }
  header button:hover { background: var(--border); }
  .theme-toggle { font-size: 1rem; padding: 0.3rem 0.5rem; }
  .mode-toggle { font-size: 1rem; padding: 0.3rem 0.5rem; }
  main { padding: 1rem; flex: 1; }
  .status, .empty { color: var(--text-secondary); font-style: italic; }
  .error { color: var(--error); white-space: pre-wrap; }
</style>
