<script lang="ts">
  // Polyfill Promise.withResolvers for older WebView versions (ES2024 feature)
  if (typeof Promise.withResolvers !== 'function') {
    Promise.withResolvers = function <T>() {
      let resolve!: (value: T | PromiseLike<T>) => void;
      let reject!: (reason?: any) => void;
      const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
      return { promise, resolve, reject };
    };
  }

  // Top-level dispatcher: given a Document variant, render the right viewer.
  // Per plan §2: "Svelte viewer components" consume "Structured JSON over IPC".
  //
  // `uri` is carried alongside the Document so Image (Phase 2.1) and Unsupported
  // (Phase 4 — "Open with…" hand-off per ADR 0004) viewers can act on it.

  import type { Document } from '@viewit/platform';
  import TextViewer from './TextViewer.svelte';
  import ImageViewer from './ImageViewer.svelte';
  import MediaViewer from './MediaViewer.svelte';
  import UnsupportedViewer from './UnsupportedViewer.svelte';
  import PlaceholderViewer from './PlaceholderViewer.svelte';
  import GridView from './GridView.svelte';
  import Onboarding from './Onboarding.svelte';
  import DebugPanel from './DebugPanel.svelte';
  import PluginStore from './PluginStore.svelte';
  import RuntimeChooser from './RuntimeChooser.svelte';
  import OfficePluginHtmlViewer from './OfficePluginHtmlViewer.svelte';
  import {
    hasAndroidBridge,
    listInstalledPlugins,
    materializeExternalUri,
    pluginSupports,
    renderDocumentWithPlugin,
    type PluginInfo,
  } from './pluginBridge';

  const OFFICE_PLUGIN_EXTS = new Set(['docx', 'docm', 'dotx', 'dotm', 'xlsx', 'xlsm', 'xls', 'pptx', 'pptm', 'potx']);
  const OFFICE_KINDS = new Set(['docx', 'xlsx', 'pptx']);

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
  let FontViewer = $state<any>(null);
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
      'stream-file': () => import('./PdfViewer.svelte'),
      'epub':     () => import('./EpubViewer.svelte'),
      'mobi':     () => import('./EpubViewer.svelte'),
      'azw3':     () => import('./EpubViewer.svelte'),
      'fictionbook': () => import('./EpubViewer.svelte'),
      'palmdoc':  () => import('./EpubViewer.svelte'),
      'archive':  () => import('./ArchiveViewer.svelte'),
      'pptx':     () => import('./PptxViewer.svelte'),
      'docx':     () => import('./DocxViewer.svelte'),
      'xlsx':     () => import('./XlsxViewer.svelte'),
      'font':     () => import('./FontViewer.svelte'),
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
      else if (k === 'pdf' || k === 'stream-file') PdfViewer = m.default;
      else if (k === 'epub' || k === 'mobi' || k === 'azw3' || k === 'fictionbook' || k === 'palmdoc') EpubViewer = m.default;
      else if (k === 'archive') ArchiveViewer = m.default;
      else if (k === 'pptx') PptxViewer = m.default;
      else if (k === 'docx') DocxViewer = m.default;
      else if (k === 'xlsx') XlsxViewer = m.default;
      else if (k === 'font') FontViewer = m.default;
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
  let docUri: string | null = $state(null);
  let pendingUri: string | null = $state(null);
  let busy = $state(false);
  let busyHint = $state('');
  let error: string | null = $state(null);
  let debugOpen = $state(false);
  let pluginStoreOpen = $state(false);
  let officeRuntimeChooserOpen = $state(false);
  let selectedOfficePlugin: PluginInfo | null = $state(null);
  let officePluginNotice = $state('');
  let officeWarnings: string[] = $state([]);
  let officeFidelity: string = $state('');
  let officeRendererLabel: string = $state('');

  import { onMount } from 'svelte';
  import {
    openFile,
    openFileFromPicker,
    openedFiles,
    onOpenedFiles,
    checkFileBeforeRead,
    pickSingleFile,
  } from '@viewit/platform';

  let imagePreviewUrl: string | null = null;
  import { theme, toggleTheme, applyTheme } from './theme.svelte';
  let _debugLog: any = null;
  async function dbg(msg: string) {
    if (!_debugLog) {
      try { _debugLog = (await import('@viewit/platform')).debugLog; } catch { _debugLog = null; }
    }
    try { _debugLog?.(msg); } catch { /* ignore */ }
  }

  function setOfficeEnhancement(doc: Document | null) {
    const d = doc as any;
    const f = d?.fidelity;
    officeWarnings = Array.isArray(d?.warnings) ? d.warnings.slice() : [];
    officeFidelity = f?.level ? `${f.level} — supports: ${(f.supports ?? []).join(', ') || 'none'}` : '';
    officeRendererLabel = d?.renderer?.label ?? '';
  }

  function isPluginRendered(): boolean {
    return Boolean((doc as any)?.renderer?.id);
  }

  function isOfficePluginHtml(): boolean {
    const d = doc as any;
    return Boolean(d?.renderer?.id === 'office-ooxml' && d?.html);
  }

  $effect(() => {
    setOfficeEnhancement(doc);
  });
  async function drainOpenedQueue() {
    try {
      const cold = await openedFiles();
      if (cold.length > 0) {
        pendingUri = cold[0];
        await load();
        return true;
      }
    } catch (e) {
      const { debugLog } = await import('@viewit/platform');
      debugLog(`openedFiles err: ${e instanceof Error ? e.message : String(e)}`);
    }
    return false;
  }

  onMount(async () => {
    applyTheme();
    docUri = initialFile ?? null;
    pendingUri = initialFile ?? null;
    const { debugLog } = await import('@viewit/platform');
    debugLog('ViewIt ready');
    if (!(await drainOpenedQueue())) {
      for (let i = 0; i < 24; i++) {
        await new Promise((r) => setTimeout(r, 250));
        if (await drainOpenedQueue()) break;
      }
    }
    onOpenedFiles((urls) => {
      if (urls.length > 0) {
        debugLog(`opened event ${urls[0].slice(0, 60)}…`);
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
    busyHint = 'Opening…';
    try {
      doc = await openWithDefaultRuntime(pendingUri);
    } catch (e: any) {
      error = e?.toString?.() ?? String(e);
    } finally {
      busy = false;
      busyHint = '';
    }
  }

  async function pick() {
    const f = await pickSingleFile();
    if (f) await pickFile(f);
  }

  async function pickFile(f: File) {
    if (imagePreviewUrl) {
      URL.revokeObjectURL(imagePreviewUrl);
      imagePreviewUrl = null;
    }
    busy = true;
    error = null;
    const viewitUri = (f as File & { viewitUri?: string }).viewitUri;
    if (viewitUri) {
      pendingUri = viewitUri;
      docUri = viewitUri;
      busyHint = 'Opening…';
      try {
        doc = await openWithDefaultRuntime(viewitUri);
      } catch (e: unknown) {
        error = e instanceof Error ? e.message : String(e);
        doc = null;
      } finally {
        busy = false;
        busyHint = '';
      }
      return;
    }
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
      selectedOfficePlugin = null;
      officePluginNotice = '';
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

  function officeExt(): string {
    const kind = (doc as any)?.kind;
    if (OFFICE_KINDS.has(kind)) return kind;
    const uri = docUri ?? pendingUri ?? '';
    return extFromUri(uri);
  }

  function extFromUri(uri: string): string {
    return uri.split(/[?#]/, 1)[0].split('.').pop()?.toLowerCase() ?? '';
  }

  const RUNTIME_PREF_KEY = 'viewit.runtimePref';
  function loadRuntimePrefs(): Record<string, string> {
    try {
      return JSON.parse(localStorage.getItem(RUNTIME_PREF_KEY) ?? '{}');
    } catch { return {}; }
  }
  function saveRuntimePref(ext: string, pluginId: string | null) {
    const prefs = loadRuntimePrefs();
    if (pluginId) prefs[ext] = pluginId; else delete prefs[ext];
    try { localStorage.setItem(RUNTIME_PREF_KEY, JSON.stringify(prefs)); } catch { /* ignore */ }
  }
  function getRuntimePref(ext: string): string | null {
    return loadRuntimePrefs()[ext] ?? null;
  }

  async function openWithDefaultRuntime(uri: string): Promise<Document> {
    selectedOfficePlugin = null;
    officePluginNotice = '';
    officeWarnings = [];
    officeFidelity = '';
    const ext = extFromUri(uri);
    let failedPluginId = '';
    let readableUri = uri;
    if (hasAndroidBridge() && OFFICE_PLUGIN_EXTS.has(ext)) {
      try {
        readableUri = materializeExternalUri(uri, ext);
      } catch (e) {
        await dbg(`runtime[${ext}] materialize failed: ${e instanceof Error ? e.message : String(e)}`);
        return {
          kind: 'unsupported',
          format: ext,
          reason: 'Android blocked direct access to this raw file path. Open it from the system file picker or share sheet so ViewIt receives a readable content URI.',
          suggestion: 'none',
        } as Document;
      }
      const prefId = getRuntimePref(ext);
      if (prefId === '__builtin__') {
        await dbg(`runtime[${ext}] pref=builtin → skipping plugin`);
      } else {
        const plugins = await listInstalledPlugins();
        const plugin = prefId
          ? plugins.find((p) => p.id === prefId && pluginSupports(p, ext))
          : plugins.find((candidate) => pluginSupports(candidate, ext));
        if (plugin) {
          await dbg(`runtime[${ext}] pref=${prefId ?? 'auto'} → plugin ${plugin.id}`);
          try {
            busyHint = `Opening with ${plugin.name}…`;
            const rendered = await renderDocumentWithPlugin(plugin, readableUri, ext) as Document;
            await dbg(`runtime[${ext}] plugin ${plugin.id} returned kind=${(rendered as any)?.kind} renderer=${(rendered as any)?.renderer?.id ?? 'none'}`);
            selectedOfficePlugin = plugin;
            officePluginNotice = `Rendered by ${plugin.name}.`;
            return rendered;
          } catch (e) {
            await dbg(`runtime[${ext}] plugin ${plugin.id} failed: ${e instanceof Error ? e.message : String(e)}`);
            officePluginNotice = `${plugin.name} could not render this file: ${e instanceof Error ? e.message : String(e)}. Using the built-in lightweight viewer.`;
            selectedOfficePlugin = null;
            failedPluginId = plugin.id;
          }
        } else if (prefId) {
          await dbg(`runtime[${ext}] pref=${prefId} missing → fallback auto/builtin`);
        }
      }
    }
    const builtIn = await openFile(readableUri);
    const detectedKind = builtIn.kind;
    if (hasAndroidBridge() && OFFICE_KINDS.has(detectedKind)) {
      const prefId = getRuntimePref(detectedKind);
      if (prefId !== '__builtin__') {
        const plugin = (await listInstalledPlugins()).find((candidate) => candidate.id !== failedPluginId && pluginSupports(candidate, detectedKind));
        if (plugin) {
          await dbg(`runtime[${detectedKind}] retry plugin ${plugin.id} via detected kind`);
          try {
            busyHint = `Opening with ${plugin.name}…`;
            const rendered = await renderDocumentWithPlugin(plugin, readableUri, detectedKind) as Document;
            await dbg(`runtime[${detectedKind}] plugin ${plugin.id} returned kind=${(rendered as any)?.kind} renderer=${(rendered as any)?.renderer?.id ?? 'none'}`);
            selectedOfficePlugin = plugin;
            officePluginNotice = `Rendered by ${plugin.name}.`;
            return rendered;
          } catch (e) {
            await dbg(`runtime[${detectedKind}] retry plugin ${plugin.id} failed: ${e instanceof Error ? e.message : String(e)}`);
            officePluginNotice = `${plugin.name} could not render this file: ${e instanceof Error ? e.message : String(e)}. Using the built-in lightweight viewer.`;
            selectedOfficePlugin = null;
          }
        }
      }
    }
    return builtIn;
  }

  async function chooseOfficePlugin(plugin: PluginInfo) {
    if (!docUri) return;
    busy = true;
    busyHint = `Opening with ${plugin.name}…`;
    error = null;
    selectedOfficePlugin = plugin;
    officePluginNotice = '';
    try {
      const ext = officeExt();
      const readableUri = materializeExternalUri(docUri, ext);
      doc = await renderDocumentWithPlugin(plugin, readableUri, ext) as Document;
      await dbg(`runtime[${officeExt()}] manual plugin ${plugin.id} returned kind=${(doc as any)?.kind} renderer=${(doc as any)?.renderer?.id ?? 'none'}`);
      officePluginNotice = `Rendered by ${plugin.name}.`;
      saveRuntimePref(ext, plugin.id);
    } catch (e) {
      officePluginNotice = `${plugin.name} could not render this file: ${e instanceof Error ? e.message : String(e)}. Using the built-in lightweight viewer.`;
      if (pendingUri) doc = await openFile(pendingUri);
      selectedOfficePlugin = null;
    } finally {
      busy = false;
      busyHint = '';
    }
  }

  async function chooseBuiltInOfficeRuntime() {
    const uri = docUri ?? pendingUri;
    if (!uri) return;
    busy = true;
    busyHint = 'Opening with built-in lightweight viewer…';
    error = null;
    selectedOfficePlugin = null;
    officePluginNotice = '';
    try {
      doc = await openFile(materializeExternalUri(uri, officeExt()));
      officePluginNotice = 'Rendered by built-in lightweight viewer.';
      saveRuntimePref(officeExt(), '__builtin__');
    } catch (e) {
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
      {#if root === 'mobile'}
        <button class="plugin-btn" onclick={() => pluginStoreOpen = true} aria-label="Plugin store" title="Plugin store">
          ⚡
        </button>
      {/if}
      <button onclick={pick}>Open file…</button>
      <button
        type="button"
        class="hint-btn"
        title="Tap for on-device debug log (replaces Chrome inspect)"
        onclick={() => (debugOpen = !debugOpen)}
      >{debugOpen ? '▾ log' : '▸ log'}</button>
      <button class="mode-toggle" onclick={() => mode = mode === 'view' ? 'browse' : 'view'} aria-label="Toggle browse mode" title="Toggle browse mode">
        {mode === 'view' ? '▦' : '↩'}
      </button>
    </div>
  </header>

  <main>
    {#if mode === 'browse'}
      <GridView root={root} onPick={(f) => { mode = 'view'; pickFile(f); }} />
    {:else if !busy && !error && !doc}
      <Onboarding />
      <p class="empty">Drop a file or pick one — everything opens.</p>
      <GridView root={root} onPick={(f) => { mode = 'view'; pickFile(f); }} />
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
      {:else if doc.kind === 'stream-file'}
        {#if PdfViewer}
          <PdfViewer document={doc} source_uri={docUri ?? ''} />
        {:else}
          <p class="status">Loading PDF viewer…</p>
        {/if}
      {:else if doc.kind === 'media'}
        <MediaViewer uri={docUri ?? ''} {...(doc as any)} />
      {:else if doc.kind === 'pdf' && PdfViewer}
        <PdfViewer document={doc} source_uri={docUri ?? ''} />
      {:else if (doc.kind === 'epub' || doc.kind === 'mobi' || doc.kind === 'azw3' || doc.kind === 'fictionbook' || doc.kind === 'palmdoc') && EpubViewer}
        <EpubViewer {...(doc as any)} />
      {:else if doc.kind === 'archive' && ArchiveViewer}
        <ArchiveViewer {...(doc as any)} />
      {:else if isOfficePluginHtml()}
        <div class="plugin-renderer-shell">
        <div class="plugin-renderer-banner">PLUGIN RENDERER ACTIVE · {officeRendererLabel || selectedOfficePlugin?.name || 'Office plugin'} · HTML VIEWER MODE</div>
        <div class="runtime-bar plugin-runtime-bar">
          <button type="button" onclick={() => officeRuntimeChooserOpen = true}>Runtime: {selectedOfficePlugin?.name ?? 'Office plugin'}</button>
          {#if officePluginNotice}<span>{officePluginNotice}</span>{/if}
          {#if officeFidelity}<span class="fidelity">{officeFidelity}</span>{/if}
          {#if officeWarnings.length > 0}<details class="warnings"><summary>{officeWarnings.length} warning(s)</summary><ul>{#each officeWarnings as w}<li>{w}</li>{/each}</ul></details>{/if}
        </div>
        <OfficePluginHtmlViewer document={doc} />
        </div>
      {:else if doc.kind === 'pptx' && PptxViewer}
        <div class:plugin-renderer-shell={isPluginRendered()}>
        {#if isPluginRendered()}<div class="plugin-renderer-banner">PLUGIN RENDERER ACTIVE · {officeRendererLabel || selectedOfficePlugin?.name || 'Office plugin'} · LIGHT PINK TEST MODE</div>{/if}
        <div class="runtime-bar" class:plugin-runtime-bar={isPluginRendered()}>
          <button type="button" onclick={() => officeRuntimeChooserOpen = true}>Runtime: {selectedOfficePlugin?.name ?? 'Built-in lightweight viewer'}</button>
          {#if officePluginNotice}<span>{officePluginNotice}</span>{/if}
          {#if officeFidelity}<span class="fidelity">{officeFidelity}</span>{/if}
          {#if officeWarnings.length > 0}<details class="warnings"><summary>{officeWarnings.length} warning(s)</summary><ul>{#each officeWarnings as w}<li>{w}</li>{/each}</ul></details>{/if}
        </div>
        {#if (doc as any).html}<div class="plugin-html-surface">{@html (doc as any).html}</div>{:else}<PptxViewer document={doc} source_uri={docUri ?? ''} />{/if}
        </div>
      {:else if doc.kind === 'docx' && DocxViewer}
        <div class:plugin-renderer-shell={isPluginRendered()}>
        {#if isPluginRendered()}<div class="plugin-renderer-banner">PLUGIN RENDERER ACTIVE · {officeRendererLabel || selectedOfficePlugin?.name || 'Office plugin'} · LIGHT PINK TEST MODE</div>{/if}
        <div class="runtime-bar" class:plugin-runtime-bar={isPluginRendered()}>
          <button type="button" onclick={() => officeRuntimeChooserOpen = true}>Runtime: {selectedOfficePlugin?.name ?? 'Built-in lightweight viewer'}</button>
          {#if officePluginNotice}<span>{officePluginNotice}</span>{/if}
          {#if officeFidelity}<span class="fidelity">{officeFidelity}</span>{/if}
          {#if officeWarnings.length > 0}<details class="warnings"><summary>{officeWarnings.length} warning(s)</summary><ul>{#each officeWarnings as w}<li>{w}</li>{/each}</ul></details>{/if}
        </div>
        {#if (doc as any).html}<div class="plugin-html-surface">{@html (doc as any).html}</div>{:else}<DocxViewer document={doc} />{/if}
        </div>
      {:else if doc.kind === 'xlsx' && XlsxViewer}
        <div class:plugin-renderer-shell={isPluginRendered()}>
        {#if isPluginRendered()}<div class="plugin-renderer-banner">PLUGIN RENDERER ACTIVE · {officeRendererLabel || selectedOfficePlugin?.name || 'Office plugin'} · LIGHT PINK TEST MODE</div>{/if}
        <div class="runtime-bar" class:plugin-runtime-bar={isPluginRendered()}>
          <button type="button" onclick={() => officeRuntimeChooserOpen = true}>Runtime: {selectedOfficePlugin?.name ?? 'Built-in lightweight viewer'}</button>
          {#if officePluginNotice}<span>{officePluginNotice}</span>{/if}
          {#if officeFidelity}<span class="fidelity">{officeFidelity}</span>{/if}
          {#if officeWarnings.length > 0}<details class="warnings"><summary>{officeWarnings.length} warning(s)</summary><ul>{#each officeWarnings as w}<li>{w}</li>{/each}</ul></details>{/if}
        </div>
        {#if (doc as any).html}<div class="plugin-html-surface">{@html (doc as any).html}</div>{:else}<XlsxViewer document={doc} />{/if}
        </div>
      {:else if doc.kind === 'unsupported'}
        <UnsupportedViewer uri={docUri ?? undefined} document={doc} />
      {:else if doc.kind === 'font' && FontViewer}
        <FontViewer {...(doc as any)} />
      {:else if doc.kind === 'placeholder'}
        <PlaceholderViewer document={doc} />
      {:else}
        <p class="error">Unknown document kind: <code>{(doc as any).kind}</code></p>
      {/if}
    {:else}
      <p class="empty">Drop a file or pick one — everything opens.</p>
    {/if}
  </main>
  <DebugPanel bind:open={debugOpen} />
  <PluginStore open={pluginStoreOpen} onClose={() => pluginStoreOpen = false} />
  <RuntimeChooser
    open={officeRuntimeChooserOpen}
    uri={docUri ?? ''}
    name={(docUri ?? 'Office file').split('/').pop() ?? 'Office file'}
    ext={officeExt()}
    builtInLabel="Built-in lightweight Office viewer"
    onClose={() => officeRuntimeChooserOpen = false}
    onUseBuiltIn={chooseBuiltInOfficeRuntime}
    onUseInstalledPlugin={chooseOfficePlugin}
  />
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
  header {
    display: flex; align-items: center; justify-content: space-between;
    padding: calc(0.5rem + env(safe-area-inset-top, 0px)) max(1rem, env(safe-area-inset-right, 0px)) 0.5rem max(1rem, env(safe-area-inset-left, 0px));
    border-bottom: 1px solid var(--border); background: var(--bg-primary);
    position: sticky; top: 0; z-index: 10;
  }
  header h1 { font-size: 1.2rem; margin: 0; letter-spacing: -0.01em; color: var(--text-primary); }
  .header-actions { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .hint-btn { font-size: 0.65rem; padding: 0.25rem 0.45rem; }
  header button { cursor: pointer; background: var(--bg-secondary); color: var(--text-primary); border: 1px solid var(--border); border-radius: 0.3rem; padding: 0.3rem 0.7rem; font-size: 0.85rem; }
  header button:hover { background: var(--border); }
  .theme-toggle { font-size: 1rem; padding: 0.3rem 0.5rem; }
  .plugin-btn { font-size: 1rem; padding: 0.3rem 0.5rem; }
  .mode-toggle { font-size: 1rem; padding: 0.3rem 0.5rem; }
  main {
    padding: 1rem max(1rem, env(safe-area-inset-right, 0px)) max(1rem, env(safe-area-inset-bottom, 0px)) max(1rem, env(safe-area-inset-left, 0px));
    flex: 1;
  }
  .status, .empty { color: var(--text-secondary); font-style: italic; }
  .error { color: var(--error); white-space: pre-wrap; }
  .runtime-bar { display: flex; gap: 0.75rem; align-items: center; flex-wrap: wrap; margin: 0 0 0.75rem; color: var(--text-secondary); font-size: 0.8rem; }
  .runtime-bar button { cursor: pointer; border: 1px solid var(--border); border-radius: 999px; background: var(--bg-secondary); color: var(--text-primary); padding: 0.35rem 0.7rem; }
  .runtime-bar span { flex: 1 1 18rem; }
  .runtime-bar .fidelity { font-style: italic; opacity: 0.8; font-size: 0.72rem; }
  .runtime-bar .warnings { font-size: 0.72rem; color: var(--text-secondary); }
  .runtime-bar .warnings summary { cursor: pointer; }
  .runtime-bar .warnings ul { margin: 0.25rem 0 0; padding-left: 1rem; max-width: 40rem; }
  .plugin-renderer-shell {
    background: #ffe6f1;
    border: 3px solid #ff5aa5;
    border-radius: 1rem;
    padding: 0.8rem;
    box-shadow: 0 0 0 0.3rem rgba(255, 90, 165, 0.15);
  }
  .plugin-renderer-banner {
    margin: 0 0 0.75rem;
    padding: 0.55rem 0.75rem;
    border-radius: 0.7rem;
    background: #ffb8d8;
    color: #65002f;
    font-weight: 900;
    font-size: 0.78rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    border: 1px solid #ff5aa5;
  }
  .plugin-runtime-bar {
    background: #ffd3e8;
    border: 1px solid #ff8cc3;
    border-radius: 999px;
    padding: 0.4rem 0.55rem;
  }
  .plugin-html-surface {
    overflow: auto;
    border-radius: 0.75rem;
  }
</style>
