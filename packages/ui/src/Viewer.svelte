<script lang="ts">
  type PromiseResolvers = {
    withResolvers?<T>(): {
      promise: Promise<T>;
      resolve: (value: T | PromiseLike<T>) => void;
      reject: (reason?: unknown) => void;
    };
  };
  const PromiseCtor = Promise as PromiseConstructor & PromiseResolvers;
  if (typeof PromiseCtor.withResolvers !== "function") {
    PromiseCtor.withResolvers = function <T>() {
      let resolve!: (value: T | PromiseLike<T>) => void;
      let reject!: (reason?: unknown) => void;
      const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
      });
      return { promise, resolve, reject };
    };
  }

  // Top-level dispatcher: given a Document variant, render the right viewer.
  // Per plan §2: "Svelte viewer components" consume "Structured JSON over IPC".
  //
  // `uri` is carried alongside the Document so Image (Phase 2.1) and Unsupported
  // (Phase 4 — "Open with…" hand-off per ADR 0004) viewers can act on it.

  import { formatFromFile, normalizeFormat, type Document } from "@viewit/platform";
  import TextViewer from "./TextViewer.svelte";
  import ImageViewer from "./ImageViewer.svelte";
  import MediaViewer from "./MediaViewer.svelte";
  import PlayerBaseHost from "./PlayerBaseHost.svelte";
  import EditorBaseHost from "./EditorBaseHost.svelte";
  import UnsupportedViewer from "./UnsupportedViewer.svelte";
  import PlaceholderViewer from "./PlaceholderViewer.svelte";
  import GridView from "./GridView.svelte";
  import Onboarding from "./Onboarding.svelte";
  import DebugPanel from "./DebugPanel.svelte";
  import PluginStore from "./PluginStore.svelte";
  import RuntimeChooser from "./RuntimeChooser.svelte";
  import SettingsModal from "./SettingsModal.svelte";
  import Icon from "./Icon.svelte";
  import SearchBar from "./SearchBar.svelte";
  import { fullscreenState, setFullscreen } from "./fullscreen.svelte";
  import OfficePluginHtmlViewer from "./OfficePluginHtmlViewer.svelte";
  import {
    archiveBridgeFor,
    hasAndroidBridge,
    installPlugin,
    isJsPlugin,
    listInstalledPlugins,
    materializeExternalUri,
    pluginSupports,
    resolveAvailableFormat,
    renderDocumentWithPlugin,
    type PluginInfo,
  } from "./pluginBridge";
  import {
    ARCHIVE_EXTS,
    FONT_EXTS,
    NON_ARCHIVE_BUNDLE_EXTS,
    OFFICE_ALL_EXTS,
    OFFICE_KINDS,
    hasMeaningfulPresentationContent,
    resolveIworkFormat,
    resolveInstalledProvider,
  } from "./runtimeRouting";

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
  let PptxVanillaViewer = $state<any>(null);
  let DocxPreview = $state<any>(null);
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
    const uri = docUri ?? "";
    const isIcs = /\.ics?$/i.test(uri);
    const isVcf = /\.vcf$/i.test(uri);
    const isDesktop = /\.desktop$/i.test(uri);
    const loaders: Record<string, (() => Promise<Comp>) | undefined> = {
      markdown: () => import("./MarkdownViewer.svelte"),
      json: () => import("./JsonViewer.svelte"),
      csv: () => import("./CsvViewer.svelte"),
      pdf: () => import("./PdfViewer.svelte"),
      "stream-file": () => import("./PdfViewer.svelte"),
      epub: () => import("./EpubViewer.svelte"),
      mobi: () => import("./EpubViewer.svelte"),
      azw3: () => import("./EpubViewer.svelte"),
      archive: () => import("./ArchiveViewer.svelte"),
      pptx: () => import("./PptxViewer.svelte"),
      "pptx-vanilla": () => import("./PptxVanillaViewer.svelte"),
      docx: () => import("./DocxPreview.svelte"),
      xlsx: () => import("./XlsxViewer.svelte"),
      font: () => import("./FontViewer.svelte"),
    };
    let loader = loaders[k] as (() => Promise<Comp>) | undefined;
    if (k === "text") {
      if (isIcs) loader = () => import("./IcsViewer.svelte");
      else if (isVcf) loader = () => import("./VcfViewer.svelte");
      else if (isDesktop) loader = () => import("./DesktopEntryViewer.svelte");
      else loader = undefined;
    }
    const isOdtLike = officeExt() === "odt" || officeExt() === "ott";
    const isStructuredDocx = isOdtLike || isIworkDocument() || Array.isArray((doc as any).blocks);
    if (k === "docx" && isStructuredDocx) {
      // ODT/OTT: plugin produces Document::Docx with structured blocks —
      // render via the block-based DocxViewer instead of docx-preview.
      loader = () => import("./DocxViewer.svelte");
    }
    if (k === "pptx" && !isIworkDocument()) {
      void resolvePptxVanillaPlugin().then((p) => {
        if (p?.id !== pptxVanillaPlugin?.id) pptxVanillaPlugin = p;
        if (p) {
          selectedOfficePlugin = p;
          if (!PptxVanillaViewer) {
            import("./PptxVanillaViewer.svelte").then((m) => {
              PptxVanillaViewer = m.default;
            });
          }
        }
      });
    }
    if (!loader) return;
    loader().then((m) => {
      if (k === "markdown") MarkdownViewer = m.default;
      else if (k === "json") JsonViewer = m.default;
      else if (k === "csv") CsvViewer = m.default;
      else if (k === "pdf" || k === "stream-file") PdfViewer = m.default;
      else if (k === "epub" || k === "mobi" || k === "azw3") EpubViewer = m.default;
      else if (k === "archive") ArchiveViewer = m.default;
      else if (k === "pptx") PptxViewer = m.default;
      else if (k === "pptx-vanilla") PptxVanillaViewer = m.default;
      else if (k === "docx") {
        if (isStructuredDocx) DocxViewer = m.default;
        else DocxPreview = m.default;
      } else if (k === "xlsx") XlsxViewer = m.default;
      else if (k === "font") FontViewer = m.default;
      else if (k === "text" && isIcs) IcsViewer = m.default;
      else if (k === "text" && isVcf) VcfViewer = m.default;
      else if (k === "text" && isDesktop) DesktopEntryViewer = m.default;
    });
  });

  let mode: "view" | "browse" = $state("view");
  let headerEl: HTMLElement | null = $state(null);

  // True inside a Tauri webview (desktop/mobile); false in a plain browser.
  const isTauriHost =
    typeof window !== "undefined" && "__TAURI_INTERNALS__" in (window as unknown as object);

  let {
    root,
    initialFile = undefined,
  }: {
    root: "desktop" | "mobile" | "web";
    initialFile?: string;
  } = $props();

  let doc: Document | null = $state(null);
  let docUri: string | null = $state(null);
  let pendingUri: string | null = $state(null);
  let pendingName: string | null = $state(null);
  let pendingExt: string | null = $state(null);
  let pendingMime: string | null = $state(null);
  let pendingPlugin: string | null = $state(null);
  let busy = $state(false);
  let busyHint = $state("");
  let error: string | null = $state(null);
  let debugOpen = $state(false);
  let pluginStoreOpen = $state(false);
  let settingsOpen = $state(false);
  let searchOpen = $state(false);
  let searchQuery = $state("");
  let searchCaseSensitive = $state(false);
  let searchMatchCount = $state(0);
  let searchCurrentMatch = $state(0);
  let searchNavCounter = $state(0);

  function extractSearchableText(doc: Document | null): string {
    if (!doc) return "";
    const d = doc as any;
    if (typeof d.content === "string") return d.content;
    if (typeof d.pretty === "string") return d.pretty;
    if (typeof d.html === "string") return d.html.replace(/<[^>]*>/g, " ");
    if (Array.isArray(d.blocks)) {
      return d.blocks
        .map((b: any) => b.text ?? (b.inlines ?? []).map((i: any) => i.text ?? "").join(""))
        .join("\n");
    }
    if (Array.isArray(d.sheets)) {
      return d.sheets
        .map((s: any) => [...(s.header ?? []), ...(s.preview_rows ?? []).flat()].join("\t"))
        .join("\n");
    }
    return "";
  }

  async function onGlobalSearch(q: string, cs: boolean) {
    searchQuery = q;
    searchCaseSensitive = cs;
    if (!q) {
      searchMatchCount = 0;
      searchCurrentMatch = 0;
      return;
    }
    if (mode === "browse") {
      searchMatchCount = 0;
      searchCurrentMatch = 0;
      return;
    }
    const text = extractSearchableText(doc);
    const { findAllMatches } = await import("./search");
    const matches = findAllMatches(text, q, cs);
    searchMatchCount = matches.length;
    searchCurrentMatch = matches.length > 0 ? 1 : 0;
  }
  let officeRuntimeChooserOpen = $state(false);
  let selectedOfficePlugin: PluginInfo | null = $state(null);
  let pptxVanillaPlugin: PluginInfo | null = $state(null);
  let officePluginNotice = $state("");
  let officeWarnings: string[] = $state([]);
  let officeFidelity: string = $state("");
  let officeRendererLabel: string = $state("");
  let loadSeq = 0;

  // Full screen is chrome-level (header toggle) while the look inside is each
  // handler's own business — viewers read `fullscreenState` and adapt.
  $effect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && fullscreenState.active) setFullscreen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // ---------------------------------------------------------------------------
  // Back navigation. One priority list decides what a back press means; the
  // Android side reports presses through the bridge, desktop/web use
  // Alt+Left or the browser Back key.
  // ---------------------------------------------------------------------------
  let mainEl: HTMLElement | null = $state(null);

  // Browse context preserved across "open file → back" round trips so the
  // user lands exactly where they left: same folder, same scroll offset.
  let browsePath = $state("");
  let browseCanUp = $state(false);
  let upSignal = $state(0);
  let savedBrowseScroll = 0;
  let restoreScrollPending = false;

  function onGridListed(path: string, parent: string | null) {
    browsePath = path;
    // "/" (filesystem root) is never a useful place to go "back" to — at the
    // storage root, back must fall through to backgrounding the app.
    browseCanUp = Boolean(parent) && parent !== path && parent !== "/";
    if (restoreScrollPending) {
      restoreScrollPending = false;
      const top = savedBrowseScroll;
      void tick().then(() => requestAnimationFrame(() => mainEl?.scrollTo({ top })));
    }
  }

  function consumeBack(): boolean {
    // Overlays close first, innermost state first, app backgrounding last.
    if (debugOpen) {
      debugOpen = false;
      return true;
    }
    if (pluginStoreOpen) {
      pluginStoreOpen = false;
      return true;
    }
    if (officeRuntimeChooserOpen) {
      officeRuntimeChooserOpen = false;
      return true;
    }
    if (fullscreenState.active) {
      setFullscreen(false);
      return true;
    }
    if (doc && mode === "view") {
      mode = "browse";
      return true;
    }
    if (mode === "browse" && browseCanUp) {
      upSignal++;
      return true;
    }
    return false;
  }

  $effect(() => {
    // Report availability for the native back callback (Android). Reads of
    // the individual flags keep this effect in sync with every state change.
    const canConsume =
      debugOpen ||
      pluginStoreOpen ||
      officeRuntimeChooserOpen ||
      fullscreenState.active ||
      Boolean(doc && mode === "view") ||
      (mode === "browse" && browseCanUp);
    if (androidBridgeAvailable()) reportBackConsumer(Boolean(canConsume));
    if (typeof window !== "undefined") {
      (window as unknown as { __viewitConsumeBack?: () => boolean }).__viewitConsumeBack = () =>
        consumeBack();
    }
  });

  $effect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return;
      if ((e.altKey && e.key === "ArrowLeft") || e.key === "BrowserBack") {
        if (consumeBack()) e.preventDefault();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  import { onMount, onDestroy, tick } from "svelte";
  import { ActivationScope } from "@viewit/contracts/activation-scope";
  import {
    openFile,
    openFileFromPicker,
    openedFiles,
    onOpenedFiles,
    checkFileBeforeRead,
    pickSingleFile,
    reportBackConsumer,
    androidBridgeAvailable,
  } from "@viewit/platform";

  let imagePreviewUrl: string | null = null;
  const appScope = new ActivationScope("viewer-app");
  let documentScope = appScope.child("document-initial");

  function replaceDocumentScope(id: string): ActivationScope {
    void documentScope.dispose("document-replaced");
    documentScope = appScope.child(id);
    imagePreviewUrl = null;
    return documentScope;
  }
  import { theme, toggleTheme, applyTheme } from "./theme.svelte";
  let _debugLog: any = null;
  async function dbg(msg: string) {
    if (!_debugLog) {
      try {
        _debugLog = (await import("@viewit/platform")).debugLog;
      } catch {
        _debugLog = null;
      }
    }
    try {
      _debugLog?.(msg);
    } catch {
      /* ignore */
    }
  }

  function setOfficeEnhancement(doc: Document | null) {
    const d = doc as any;
    const f = d?.fidelity;
    officeWarnings = Array.isArray(d?.warnings) ? d.warnings.slice() : [];
    officeFidelity = f?.level
      ? `${f.level} — supports: ${(f.supports ?? []).join(", ") || "none"}`
      : "";
    officeRendererLabel = d?.renderer?.label ?? "";
  }

  function isPluginRendered(): boolean {
    return Boolean((doc as any)?.renderer?.id);
  }

  function isOfficePluginHtml(): boolean {
    const d = doc as any;
    return Boolean(d?.renderer?.id === "office-ooxml" && d?.html);
  }

  $effect(() => {
    setOfficeEnhancement(doc);
  });
  async function drainOpenedQueue() {
    try {
      let cold = await openedFiles();
      const bridge = (window as any).AndroidBridge;
      if (cold.length === 0 && bridge && typeof bridge.drainPendingOpenUris === "function") {
        try {
          const pending = JSON.parse(bridge.drainPendingOpenUris());
          if (Array.isArray(pending)) cold = pending;
        } catch {
          cold = [];
        }
      }
      if (cold.length > 0) {
        setPendingOpen(cold[0]);
        await load();
        return true;
      }
    } catch (e) {
      const { debugLog } = await import("@viewit/platform");
      debugLog(`openedFiles err: ${e instanceof Error ? e.message : String(e)}`);
    }
    return false;
  }

  /** Best-effort real display name for a picker File across Android OEM providers */
  function resolvePickerName(f: File, viewitUri: string): string | null {
    const base = f.name;
    if (base && base !== "file" && base !== "picked-file" && !/^\d+$/.test(base)) {
      return base;
    }
    try {
      const bridge = (window as any).AndroidBridge;
      if (bridge && typeof bridge.getDisplayName === "function") {
        const name = bridge.getDisplayName(viewitUri);
        if (name && name !== "file" && !/^\d+$/.test(name)) return name;
      }
    } catch {
      /* ignore */
    }
    return base || null;
  }

  function setPendingOpen(record: unknown) {
    if (typeof record === "string") {
      pendingUri = record;
      pendingName = null;
      pendingExt = null;
      pendingMime = null;
      pendingPlugin = null;
    } else if (record && typeof record === "object") {
      const r = record as any;
      pendingUri = typeof r.uri === "string" ? r.uri : null;
      pendingName = typeof r.name === "string" && r.name ? r.name : null;
      pendingExt = typeof r.ext === "string" && r.ext ? r.ext : null;
      pendingMime = typeof r.mime === "string" && r.mime ? r.mime : null;
      pendingPlugin = typeof r.plugin === "string" && r.plugin ? r.plugin : null;
    } else {
      pendingUri = null;
    }
  }

  onMount(async () => {
    applyTheme();
    if (root === "desktop" && "__TAURI_INTERNALS__" in window) {
      const { invoke } = await import("@tauri-apps/api/core");
      const report = async () => {
        const media = document.querySelector<HTMLMediaElement>("audio, video");
        const errorElement = [
          ...document.querySelectorAll<HTMLElement>(".err, .error, .error-card"),
        ].find(
          (element) => !element.closest(".editor-details") && element.getClientRects().length > 0,
        );
        const payload = JSON.stringify({
          body: document.body.innerText.slice(0, 1000),
          error: errorElement?.textContent?.trim() ?? "",
          controls: Boolean(document.querySelector(".controls")),
          media: Boolean(media),
          mediaSrc: media?.getAttribute("src") ?? "",
          mediaError: media?.error?.message ?? "",
          archive: Boolean(document.querySelector(".archive-viewer")),
          pdf: Boolean(document.querySelector(".pdf-viewer")),
          docx: Boolean(document.querySelector(".docx-viewer")),
          xlsx: Boolean(document.querySelector(".xlsx-viewer")),
          pptx: Boolean(document.querySelector(".pptx-root")),
        });
        return invoke<boolean>("desktop_e2e_report", { payload }).catch(() => false);
      };
      if (await report()) {
        const timer = setInterval(() => void report(), 500);
        appScope.register("desktop-e2e-report", () => clearInterval(timer));
      }
    }
    docUri = initialFile ?? null;
    pendingUri = initialFile ?? null;
    const { debugLog } = await import("@viewit/platform");
    debugLog("ViewIt ready");
    if (!(await drainOpenedQueue())) {
      for (let i = 0; i < 24; i++) {
        await new Promise((r) => setTimeout(r, 250));
        if (await drainOpenedQueue()) break;
      }
    }
    (window as any).__viewitAndroidOpened = (urls: any) => {
      const list = Array.isArray(urls) ? urls : [urls];
      if (list.length > 0) {
        setPendingOpen(list[0]);
        debugLog(`android opened event ${String(pendingUri).slice(0, 60)}…`);
        void load();
      }
    };
    appScope.register("android-open-handler", () => {
      delete (window as any).__viewitAndroidOpened;
    });
    const unlisten = await onOpenedFiles((urls) => {
      if (urls.length > 0) {
        setPendingOpen(urls[0]);
        debugLog(`opened event ${String(pendingUri).slice(0, 60)}…`);
        void load();
      }
    });
    appScope.register("opened-files-listener", unlisten);
    const poll = setInterval(() => {
      void drainOpenedQueue();
    }, 1500);
    appScope.register("opened-files-poll", () => clearInterval(poll));
  });

  // Expose the header's rendered height as a CSS custom property so child
  // viewers (PDF sidebar, etc.) can sticky-offset below it.
  $effect(() => {
    if (!headerEl) return;
    const ro = new ResizeObserver(([entry]) => {
      const borderBox = Array.isArray(entry.borderBoxSize)
        ? entry.borderBoxSize[0]
        : entry.borderBoxSize;
      document.documentElement.style.setProperty(
        "--header-h",
        `${borderBox?.blockSize ?? entry.contentRect.height}px`,
      );
    });
    ro.observe(headerEl);
    return () => ro.disconnect();
  });

  onDestroy(() => {
    void appScope.dispose("viewer-destroyed");
  });

  async function load() {
    if (!pendingUri) return;
    const uri = pendingUri;
    const seq = ++loadSeq;
    replaceDocumentScope(`document:${seq}`);
    const nameHint = pendingName ?? undefined;
    const extHint = pendingExt ?? undefined;
    const mimeHint = pendingMime ?? undefined;
    const pluginHint = pendingPlugin ?? undefined;
    busy = true;
    setFullscreen(false);
    error = null;
    doc = null;
    docUri = uri;
    busyHint = "Opening…";
    try {
      const nextDoc = await openWithDefaultRuntime(uri, nameHint, extHint, pluginHint, mimeHint);
      if (seq !== loadSeq || pendingUri !== uri) return;
      doc = nextDoc;
      const nextKind = (nextDoc as any)?.kind;
      const nextExt =
        extHint ||
        extFromUri(uri, nameHint) ||
        (nextKind === "text"
          ? "txt"
          : nextKind === "markdown"
            ? "md"
            : nextKind === "json"
              ? "json"
              : "");
      if (nextKind === "media") void resolvePlayBase((nextDoc as any)?.ext ?? nextExt);
      if (["text", "markdown", "json"].includes(nextKind)) void resolveEditBase(nextExt);
    } catch (e: any) {
      if (seq !== loadSeq || pendingUri !== uri) return;
      error = e?.toString?.() ?? String(e);
      doc = null;
    } finally {
      if (seq === loadSeq && pendingUri === uri) {
        busy = false;
        busyHint = "";
      }
    }
  }

  async function pick() {
    const f = await pickSingleFile();
    if (f) await pickFile(f);
  }

  async function pickFile(f: File) {
    busy = true;
    setFullscreen(false);
    error = null;
    // Leaving the browse screen: remember exactly where the user was so a
    // back press restores this folder and scroll offset.
    if (mode === "browse") {
      savedBrowseScroll = mainEl?.scrollTop ?? 0;
      restoreScrollPending = browsePath !== "";
      mode = "view";
    }
    const viewitUri = (f as File & { viewitUri?: string }).viewitUri;
    if (viewitUri) {
      const seq = ++loadSeq;
      replaceDocumentScope(`document:${seq}`);
      pendingUri = viewitUri;
      pendingName = resolvePickerName(f, viewitUri);
      pendingExt = null;
      pendingMime = null;
      pendingPlugin = null;
      docUri = viewitUri;
      doc = null;
      busyHint = "Opening…";
      try {
        const nextDoc = await openWithDefaultRuntime(
          viewitUri,
          pendingName ?? undefined,
          undefined,
          undefined,
        );
        if (seq !== loadSeq || pendingUri !== viewitUri) return;
        doc = nextDoc;
      } catch (e: unknown) {
        if (seq !== loadSeq || pendingUri !== viewitUri) return;
        error = e instanceof Error ? e.message : String(e);
        doc = null;
      } finally {
        if (seq === loadSeq && pendingUri === viewitUri) {
          busy = false;
          busyHint = "";
        }
      }
      return;
    }
    const seq = ++loadSeq;
    const scope = replaceDocumentScope(`document:${seq}`);
    pendingUri = f.name;
    pendingName = f.name;
    pendingExt = formatFromFile(f.name, f.type);
    pendingMime = f.type || null;
    pendingPlugin = null;
    docUri = f.name;
    doc = null;
    const mb = (f.size / 1_048_576).toFixed(1);
    try {
      const gate = checkFileBeforeRead(f);
      if (gate.reject) {
        doc = {
          kind: "unsupported",
          format: "unsupported",
          reason: gate.reason,
          suggestion: gate.openWithExternal ? "open-with-external" : "none",
        };
        return;
      }
      busyHint = `Reading ${f.name} (${mb} MB)…`;
      const nextDoc = await openFileFromPicker(f);
      if (seq !== loadSeq || pendingUri !== f.name) return;
      doc = nextDoc;
      selectedOfficePlugin = null;
      officePluginNotice = "";
      // On the web (no Tauri host), the raw bytes only live in this File — give
      // the viewers a `blob:` URL they can actually load for image/media/pdf/font.
      const needsObjectUrl =
        !isTauriHost &&
        ["image", "media", "pdf", "font", "epub", "mobi", "azw3"].includes(doc.kind);
      if (needsObjectUrl) {
        if (imagePreviewUrl) {
          URL.revokeObjectURL(imagePreviewUrl);
          imagePreviewUrl = null;
        }
        const objectUrl = URL.createObjectURL(f);
        scope.ownObjectUrl(objectUrl);
        if (doc.kind === "image") imagePreviewUrl = objectUrl;
        docUri = objectUrl;
      }
    } catch (e: unknown) {
      if (seq !== loadSeq || pendingUri !== f.name) return;
      error = e instanceof Error ? e.message : String(e);
      doc = null;
    } finally {
      if (seq === loadSeq && pendingUri === f.name) {
        busy = false;
        busyHint = "";
      }
    }
  }

  function officeExt(): string {
    const uri = docUri ?? pendingUri ?? "";
    const derived = extFromUri(uri, pendingName ?? undefined);
    if (derived && OFFICE_ALL_EXTS.has(derived)) return derived;
    const kind = (doc as any)?.kind;
    if (OFFICE_KINDS.has(kind)) return kind;
    return derived;
  }

  function iworkExt(): "pages" | "numbers" | "key" | null {
    const ext =
      normalizeFormat(pendingExt) ||
      extFromUri(docUri ?? pendingUri ?? "", pendingName ?? undefined);
    return resolveIworkFormat(ext, (doc as any)?.source_format);
  }

  function isIworkDocument(): boolean {
    return iworkExt() !== null;
  }

  function iworkLabel(): string {
    const labels = { pages: "Apple Pages", numbers: "Apple Numbers", key: "Apple Keynote" };
    const ext = iworkExt();
    return ext ? labels[ext] : "Apple iWork";
  }

  function extFromUri(uri: string, nameHint?: string): string {
    const fromName = nameHint ? (nameHint.split(".").pop()?.toLowerCase() ?? "") : "";
    if (fromName && fromName.length <= 8 && /^[a-z0-9]+$/.test(fromName)) return fromName;
    const fromPath = uri.split(/[?#]/, 1)[0].split(".").pop()?.toLowerCase() ?? "";
    if (fromPath && fromPath.length <= 8 && /^[a-z0-9]+$/.test(fromPath)) return fromPath;
    if (hasAndroidBridge()) {
      try {
        const bridge = (window as any).AndroidBridge;
        const mime = typeof bridge.getMimeType === "function" ? bridge.getMimeType(uri) : "";
        const fromMime = extFromMime(mime);
        if (fromMime) return fromMime;
      } catch {
        /* ignore */
      }
    }
    return "";
  }

  /** Normalize a plugin-returned format id ("zip", "tar.gz", …) into the
   *  Format union so other viewers can switch on it. Unknown id is passed
   *  through for display only. */
  function toArchiveFormat(value: string): string {
    const v = value.toLowerCase();
    if (v === "zip") return "archive-zip";
    if (v === "tar") return "archive-tar";
    if (v === "rar") return "archive-rar";
    if (v === "7z") return "archive-7z";
    if (
      v === "gz" ||
      v === "tgz" ||
      v === "tar.gz" ||
      v === "tbz2" ||
      v === "tzst" ||
      v === "txz" ||
      v === "tlz" ||
      v.includes(".tar")
    )
      return "archive-tar-gz";
    return v;
  }

  function extFromMime(mime: string): string {
    const normalized = mime.toLowerCase();
    const map: Record<string, string> = {
      "application/vnd.openxmlformats-officedocument.wordprocessingml.document": "docx",
      "application/vnd.ms-word.document.macroenabled.12": "docm",
      "application/vnd.openxmlformats-officedocument.wordprocessingml.template": "dotx",
      "application/vnd.ms-word.template.macroenabled.12": "dotm",
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": "xlsx",
      "application/vnd.ms-excel.sheet.macroenabled.12": "xlsm",
      "application/vnd.ms-excel.sheet.binary.macroenabled.12": "xlsb",
      "application/vnd.openxmlformats-officedocument.presentationml.presentation": "pptx",
      "application/vnd.ms-powerpoint.presentation.macroenabled.12": "pptm",
      "application/vnd.ms-powerpoint.template.macroenabled.12": "potm",
      "application/vnd.openxmlformats-officedocument.presentationml.template": "potx",
      "application/vnd.oasis.opendocument.text": "odt",
      "application/vnd.oasis.opendocument.text-template": "ott",
      "application/vnd.oasis.opendocument.spreadsheet": "ods",
      "application/vnd.oasis.opendocument.spreadsheet-template": "ots",
      "application/vnd.oasis.opendocument.presentation": "odp",
      "application/vnd.oasis.opendocument.presentation-template": "otp",
      "application/msword": "doc",
      "application/vnd.ms-excel": "xls",
      "application/vnd.ms-powerpoint": "ppt",
    };
    return map[normalized] ?? "";
  }

  const RUNTIME_PREF_KEY = "viewit.runtimePref";
  function loadRuntimePrefs(): Record<string, string> {
    try {
      return JSON.parse(localStorage.getItem(RUNTIME_PREF_KEY) ?? "{}");
    } catch {
      return {};
    }
  }
  function saveRuntimePref(ext: string, pluginId: string | null) {
    const prefs = loadRuntimePrefs();
    if (pluginId) prefs[ext] = pluginId;
    else delete prefs[ext];
    try {
      localStorage.setItem(RUNTIME_PREF_KEY, JSON.stringify(prefs));
    } catch {
      /* ignore */
    }
  }

  // Reactive CTA: whenever the open document/URI changes, offer to install the
  // matching plugin if it's absent (online). Derives the extension from the URI
  // so it works on every open path (VIEW intent, picker, drop), not just load().
  async function syncInstallCta(uri: string): Promise<void> {
    if (!hasAndroidBridge() || typeof navigator === "undefined" || !navigator.onLine) {
      formatInstallCta = null;
      return;
    }
    const ext = normalizeFormat(pendingExt) || formatFromFile(pendingName || uri, pendingMime);
    if (!ext) {
      formatInstallCta = null;
      return;
    }
    try {
      const capability = await resolveAvailableFormat(ext);
      const plugin = capability.available[0] as PluginInfo | undefined;
      formatInstallCta = plugin ? { ext, pluginName: plugin.name, plugin, uri } : null;
    } catch {
      formatInstallCta = null;
    }
  }

  $effect(() => {
    const d = doc;
    const uri = docUri;
    if (!d || !uri) {
      formatInstallCta = null;
      return;
    }
    void syncInstallCta(uri);
  });

  // One-tap: install the matching plugin from the catalog, then reopen the file so
  // the richer renderer (or the plugin's viewer) takes over.
  let formatInstallCta = $state<{
    ext: string;
    pluginName: string;
    plugin: PluginInfo;
    uri: string;
  } | null>(null);
  let formatInstalling = $state(false);

  // Player base (base=play): an installed js plugin can replace the built-in media player.
  let playPlugin = $state<PluginInfo | null>(null);
  let usePlayerBase = $state(false);
  let editPlugin = $state<PluginInfo | null>(null);
  let useEditorBase = $state(false);
  async function resolveEditBase(editExt: string): Promise<PluginInfo | null> {
    const ext = editExt.toLowerCase();
    if (!ext || !hasAndroidBridge()) {
      editPlugin = null;
      return null;
    }
    const installed = await listInstalledPlugins();
    const decision = resolveInstalledProvider(installed, "viewit.document.edit", ext, null);
    const found = installed.find((plugin) => plugin.id === decision.selected?.packageId) ?? null;
    editPlugin = found;
    return found;
  }

  async function activateEditor(): Promise<void> {
    const d = doc as any;
    const ext =
      extFromUri(docUri ?? "", pendingName ?? undefined) ||
      (d?.kind === "text"
        ? "txt"
        : d?.kind === "markdown"
          ? "md"
          : d?.kind === "json"
            ? "json"
            : "");
    const found = await resolveEditBase(ext);
    if (found) {
      editPlugin = found;
      await tick();
      useEditorBase = true;
    } else {
      pluginStoreOpen = true;
    }
  }

  // Resolve play/edit bases reactively so every open path (VIEW intent, picker,
  // drop) discovers installed plugin bases, not only load().
  $effect(() => {
    const d = doc as any;
    const uri = docUri;
    const name = pendingName;
    if (!d || !uri) {
      playPlugin = null;
      usePlayerBase = false;
      editPlugin = null;
      useEditorBase = false;
      return;
    }
    const derivedExt = extFromUri(uri, name ?? undefined);
    const ext =
      derivedExt ||
      (d.kind === "text" ? "txt" : d.kind === "markdown" ? "md" : d.kind === "json" ? "json" : "");
    if (d.kind === "media") void resolvePlayBase(d.ext ?? ext);
    else {
      playPlugin = null;
      usePlayerBase = false;
    }
    if (["text", "markdown", "json"].includes(d.kind)) void resolveEditBase(ext);
    else {
      editPlugin = null;
      useEditorBase = false;
    }
  });

  async function resolvePlayBase(mediaExt: string): Promise<void> {
    const ext = mediaExt.toLowerCase();
    if (!ext || !hasAndroidBridge()) {
      playPlugin = null;
      usePlayerBase = false;
      return;
    }
    const installed = await listInstalledPlugins();
    const decision = resolveInstalledProvider(installed, "viewit.media.play", ext, null);
    playPlugin = installed.find((plugin) => plugin.id === decision.selected?.packageId) ?? null;
    usePlayerBase = false;
  }

  async function confirmFormatInstall(): Promise<void> {
    const cta = formatInstallCta;
    if (!cta || formatInstalling) return;
    formatInstalling = true;
    try {
      await installPlugin(cta.plugin);
      formatInstallCta = null;
      // reopen after the plugin is in place
      if (cta.uri) {
        pendingUri = cta.uri;
        void load();
      }
    } catch (e) {
      console.error("[viewit] plugin auto-install failed:", e);
    } finally {
      formatInstalling = false;
    }
  }

  async function resolvePptxVanillaPlugin(): Promise<PluginInfo | null> {
    if (!hasAndroidBridge()) return null;
    const prefId = getRuntimePref("pptx");
    if (prefId === "__builtin__") return null;
    const installed = await listInstalledPlugins();
    const decision = resolveInstalledProvider(installed, "viewit.document.render", "pptx", prefId, [
      { service: "viewit.document.parse", contractVersion: 1 },
    ]);
    return installed.find((plugin) => plugin.id === decision.selected?.packageId) ?? null;
  }
  function getRuntimePref(ext: string): string | null {
    return loadRuntimePrefs()[ext] ?? null;
  }

  async function openWithDefaultRuntime(
    uri: string,
    nameHint?: string,
    extHint?: string,
    pluginHint?: string,
    mimeHint?: string,
  ): Promise<Document> {
    selectedOfficePlugin = null;
    officePluginNotice = "";
    officeWarnings = [];
    officeFidelity = "";
    const ext = normalizeFormat(extHint) || formatFromFile(nameHint || uri, mimeHint);
    let failedPluginId = "";
    let readableUri = uri;
    if (hasAndroidBridge() && OFFICE_ALL_EXTS.has(ext)) {
      try {
        readableUri = materializeExternalUri(uri, ext);
      } catch (e) {
        await dbg(
          `runtime[${ext}] materialize failed: ${e instanceof Error ? e.message : String(e)}`,
        );
        return {
          kind: "unsupported",
          format: ext,
          reason:
            "Android blocked direct access to this raw file path. Open it from the system file picker or share sheet so ViewIt receives a readable content URI.",
          suggestion: "none",
        } as Document;
      }
      const prefId = getRuntimePref(ext);
      if (prefId === "__builtin__") {
        await dbg(`runtime[${ext}] pref=builtin → skipping plugin`);
      } else {
        const plugins = await listInstalledPlugins();
        // runtime=js plugins render in the WebView (Viewer template), not via
        // the native renderDocumentWithPlugin bridge — skip them here.
        const decision = resolveInstalledProvider(
          plugins.filter((candidate) => !isJsPlugin(candidate)),
          "viewit.document.parse",
          ext,
          prefId,
        );
        const plugin =
          plugins.find((candidate) => candidate.id === decision.selected?.packageId) ?? null;
        if (prefId) {
          const prefPlugin = plugins.find((p) => p.id === prefId && pluginSupports(p, ext));
          if (!prefPlugin) {
            await dbg(`runtime[${ext}] pref=${prefId} missing → fallback auto/builtin`);
          } else if (isJsPlugin(prefPlugin)) {
            // The js plugin owns WebView rendering; fetch a parseable doc from the
            // native plugin so the template's pptx branch can hand bytes to it.
            await dbg(
              `runtime[${ext}] pref=${prefId} → js plugin ${prefPlugin.id} (renders in WebView)`,
            );
          }
        }
        if (plugin) {
          await dbg(`runtime[${ext}] pref=${prefId ?? "auto"} → plugin ${plugin.id}`);
          try {
            busyHint = `Opening with ${plugin.name}…`;
            const rendered = (await renderDocumentWithPlugin(
              plugin,
              readableUri,
              ext,
              documentScope.signal,
            )) as Document;
            if ((ext === "odp" || ext === "otp") && !hasMeaningfulPresentationContent(rendered)) {
              throw new Error("Office plugin returned an empty presentation");
            }
            await dbg(
              `runtime[${ext}] plugin ${plugin.id} returned kind=${(rendered as any)?.kind} renderer=${(rendered as any)?.renderer?.id ?? "none"}`,
            );
            selectedOfficePlugin = plugin;
            officePluginNotice = `Rendered by ${plugin.name}.`;
            return rendered;
          } catch (e) {
            await dbg(
              `runtime[${ext}] plugin ${plugin.id} failed: ${e instanceof Error ? e.message : String(e)}`,
            );
            officePluginNotice = `${plugin.name} was unavailable. Using the built-in lightweight viewer.`;
            selectedOfficePlugin = null;
            failedPluginId = plugin.id;
          }
        }
      }
    }
    // Fonts — built-in font-universal native plugin (Android only). The mobile
    // base Rust lib intentionally excludes fmt-font to save APK budget, so the
    // native open() would otherwise return a placeholder. Defer to the plugin
    // whenever it is installed.
    if (hasAndroidBridge() && FONT_EXTS.has(ext)) {
      const fontPlugins = await listInstalledPlugins();
      const fontPlugin = fontPlugins.find(
        (candidate) =>
          candidate.id === "font-universal" &&
          !isJsPlugin(candidate) &&
          pluginSupports(candidate, ext),
      );
      if (fontPlugin) {
        try {
          await dbg(`font[${ext}] via ${fontPlugin.id}`);
          const rendered = (await renderDocumentWithPlugin(
            fontPlugin,
            readableUri,
            ext,
            documentScope.signal,
          )) as Document;
          if (rendered && rendered.kind !== "unsupported") {
            await dbg(
              `font[${ext}] plugin returned kind=${(rendered as any)?.kind} family=${(rendered as any)?.family_name}`,
            );
            return rendered;
          }
          await dbg(
            `font[${ext}] plugin could not parse: ${(rendered as any)?.reason ?? "unknown"}`,
          );
        } catch (e) {
          await dbg(`font[${ext}] plugin failed: ${e instanceof Error ? e.message : String(e)}`);
        }
      } else {
        await dbg(`font[${ext}] font-universal not installed; falling back to built-in`);
      }
    }
    // Archive — native compression-universal plugin. It lists every supported
    // container precisely (zip, 7z, rar, tar, gz/bz2/xz/zst/lz4/lzma, and
    // tar.* chains) without extracting first, where the browser wasm covers
    // only a subset. Prefer the plugin on Android; fall back to built-in.
    let pluginArchiveError: string | null = null;
    if (hasAndroidBridge() && ARCHIVE_EXTS.has(ext)) {
      const archivePlugins = await listInstalledPlugins();
      const archivePlugin = archivePlugins.find(
        (candidate) =>
          candidate.id === "compression-universal" &&
          !isJsPlugin(candidate) &&
          pluginSupports(candidate, ext),
      );
      if (archivePlugin) {
        const displayName = (nameHint ?? uri.split("/").pop()?.split("?")[0] ?? uri) || "archive";
        try {
          busyHint = `Reading archive with ${archivePlugin.name}…`;
          const api = archiveBridgeFor(archivePlugin, uri, displayName);
          if (!api) throw new Error("Archive bridge unavailable");
          const listing = await api.listArchive();
          if (listing.ok && listing.entries) {
            await dbg(`archive[${ext}] via ${archivePlugin.id}: ${listing.entries.length} entries`);
            return {
              kind: "archive",
              entries: listing.entries,
              format: toArchiveFormat(listing.format ?? ext),
              byte_len: 0,
              name: displayName,
              renderer: { id: archivePlugin.id, label: archivePlugin.name },
            } as Document;
          }
          pluginArchiveError = listing.error ?? "Archive could not be listed";
          await dbg(`archive[${ext}] ${archivePlugin.id} listing failed: ${pluginArchiveError}`);
        } catch (e) {
          pluginArchiveError = e instanceof Error ? e.message : String(e);
          await dbg(`archive[${ext}] ${archivePlugin.id} error: ${pluginArchiveError}`);
        }
      }
    }
    if (
      hasAndroidBridge() &&
      ext &&
      !OFFICE_ALL_EXTS.has(ext) &&
      !FONT_EXTS.has(ext) &&
      !ARCHIVE_EXTS.has(ext)
    ) {
      const capability = await resolveAvailableFormat(ext);
      if (capability.kind === "installed-plugin") {
        const installed = await listInstalledPlugins();
        const hinted = pluginHint
          ? installed.find(
              (candidate) =>
                candidate.id === pluginHint &&
                (candidate.base ?? "view") === "view" &&
                !isJsPlugin(candidate) &&
                pluginSupports(candidate, ext),
            )
          : null;
        const plugin = (hinted ?? capability.plugin) as PluginInfo;
        if (!isJsPlugin(plugin)) {
          try {
            busyHint = `Opening with ${plugin.name}…`;
            const pluginUri = materializeExternalUri(uri, ext);
            const rendered = (await renderDocumentWithPlugin(
              plugin,
              pluginUri,
              ext,
              documentScope.signal,
            )) as Document;
            if ((ext === "odp" || ext === "otp") && !hasMeaningfulPresentationContent(rendered)) {
              throw new Error("Office plugin returned an empty presentation");
            }
            if (rendered.kind !== "unsupported") return rendered;
            await dbg(`runtime[${ext}] plugin ${plugin.id} declined the file`);
          } catch (e) {
            await dbg(
              `runtime[${ext}] plugin ${plugin.id} failed: ${e instanceof Error ? e.message : String(e)}`,
            );
          }
        }
      }
    }
    let builtIn: Document;
    try {
      builtIn = await openFile(readableUri, nameHint);
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      await dbg(`openFile[${ext}] failed before plugin sniff: ${reason}`);
      builtIn = {
        kind: "unsupported",
        format: ext || "unknown",
        reason,
        suggestion: "none",
      } as Document;
    }
    // Misnamed or unrecognized archives (e.g. "archive.7z.enc") — only if the
    // built-in runtime produced a placeholder/unsupported result, let the native
    // plugin sniff the container by magic before giving up. Never preempts an
    // extension the built-in can already render (images, office, media, …).
    if (
      (builtIn.kind === "placeholder" || builtIn.kind === "unsupported") &&
      hasAndroidBridge() &&
      !pluginArchiveError &&
      !NON_ARCHIVE_BUNDLE_EXTS.has(ext)
    ) {
      try {
        const archivePlugins = await listInstalledPlugins();
        const archivePlugin = archivePlugins.find(
          (candidate) => candidate.id === "compression-universal" && !isJsPlugin(candidate),
        );
        if (archivePlugin) {
          const displayName = (nameHint ?? uri.split("/").pop()?.split("?")[0] ?? uri) || "archive";
          const api = archiveBridgeFor(archivePlugin, uri, displayName);
          if (api) {
            const detected = await api.detectFormat();
            if (detected && detected !== "unknown") {
              await dbg(`archive[${ext}] sniffed ${detected} after built-in placeholder`);
              const listing = await api.listArchive();
              if (listing.ok && listing.entries) {
                return {
                  kind: "archive",
                  entries: listing.entries,
                  format: toArchiveFormat(listing.format ?? detected),
                  byte_len: 0,
                  name: displayName,
                  renderer: { id: archivePlugin.id, label: archivePlugin.name },
                } as Document;
              }
              pluginArchiveError = listing.error ?? `Archive could not be listed (${detected})`;
              await dbg(`archive[${ext}] sniffed listing failed: ${pluginArchiveError}`);
            }
          }
        }
      } catch (e) {
        pluginArchiveError = e instanceof Error ? e.message : String(e);
        await dbg(`archive[${ext}] sniff fallback error: ${pluginArchiveError}`);
      }
    }
    // The built-in runtime emits kind 'placeholder' (not 'unsupported') for
    // archives it can't read — e.g. encrypted 7z headers. Surface the plugin's
    // reason (encrypted/password-protected/corrupted) instead of the generic
    // "Phase 1 scaffold" page in that case too.
    if ((builtIn.kind === "unsupported" || builtIn.kind === "placeholder") && pluginArchiveError) {
      return {
        kind: "unsupported",
        format: toArchiveFormat(ext),
        reason: pluginArchiveError,
        suggestion: "open-with-external",
      } as Document;
    }
    const detectedKind = builtIn.kind;
    if (hasAndroidBridge() && OFFICE_KINDS.has(detectedKind) && OFFICE_ALL_EXTS.has(ext)) {
      const prefId = getRuntimePref(detectedKind);
      if (prefId !== "__builtin__") {
        const installed = await listInstalledPlugins();
        const decision = resolveInstalledProvider(
          installed.filter(
            (candidate) => candidate.id !== failedPluginId && !isJsPlugin(candidate),
          ),
          "viewit.document.parse",
          detectedKind,
          prefId,
        );
        const plugin =
          installed.find((candidate) => candidate.id === decision.selected?.packageId) ?? null;
        if (plugin) {
          await dbg(`runtime[${detectedKind}] retry plugin ${plugin.id} via detected kind`);
          try {
            busyHint = `Opening with ${plugin.name}…`;
            const rendered = (await renderDocumentWithPlugin(
              plugin,
              readableUri,
              detectedKind,
              documentScope.signal,
            )) as Document;
            await dbg(
              `runtime[${detectedKind}] plugin ${plugin.id} returned kind=${(rendered as any)?.kind} renderer=${(rendered as any)?.renderer?.id ?? "none"}`,
            );
            selectedOfficePlugin = plugin;
            officePluginNotice = `Rendered by ${plugin.name}.`;
            return rendered;
          } catch (e) {
            await dbg(
              `runtime[${detectedKind}] retry plugin ${plugin.id} failed: ${e instanceof Error ? e.message : String(e)}`,
            );
            officePluginNotice = `${plugin.name} was unavailable. Using the built-in lightweight viewer.`;
            selectedOfficePlugin = null;
          }
        }
      }
    }
    return builtIn;
  }

  async function chooseOfficePlugin(plugin: PluginInfo) {
    if (!docUri) return;
    const uri = docUri;
    const seq = ++loadSeq;
    busy = true;
    setFullscreen(false);
    busyHint = `Opening with ${plugin.name}…`;
    error = null;
    selectedOfficePlugin = plugin;
    officePluginNotice = "";
    try {
      const ext = officeExt();
      const readableUri = materializeExternalUri(uri, ext);
      if (isJsPlugin(plugin)) {
        // JS plugins render in the WebView (no native bridge). Re-open through
        // the native plugin so doc.kind matches, then let the template hand the
        // file bytes to the js renderer.
        saveRuntimePref(ext, plugin.id);
        pptxVanillaPlugin = plugin;
        const fallback = await openFile(readableUri);
        if (seq !== loadSeq || docUri !== uri) return;
        doc = fallback;
        officePluginNotice = `WebView renderer ${plugin.name} selected.`;
        return;
      }
      const nextDoc = (await renderDocumentWithPlugin(
        plugin,
        readableUri,
        ext,
        documentScope.signal,
      )) as Document;
      if ((ext === "odp" || ext === "otp") && !hasMeaningfulPresentationContent(nextDoc)) {
        throw new Error("Office plugin returned an empty presentation");
      }
      if (seq !== loadSeq || docUri !== uri) return;
      doc = nextDoc;
      await dbg(
        `runtime[${officeExt()}] manual plugin ${plugin.id} returned kind=${(doc as any)?.kind} renderer=${(doc as any)?.renderer?.id ?? "none"}`,
      );
      officePluginNotice = `Rendered by ${plugin.name}.`;
      saveRuntimePref(ext, plugin.id);
    } catch (e) {
      if (seq !== loadSeq || docUri !== uri) return;
      officePluginNotice = `${plugin.name} was unavailable. Using the built-in lightweight viewer.`;
      if (pendingUri) doc = await openFile(pendingUri);
      selectedOfficePlugin = null;
    } finally {
      if (seq === loadSeq && docUri === uri) {
        busy = false;
        busyHint = "";
      }
    }
  }

  async function chooseBuiltInOfficeRuntime() {
    const uri = docUri ?? pendingUri;
    if (!uri) return;
    const seq = ++loadSeq;
    busy = true;
    setFullscreen(false);
    busyHint = "Opening with built-in lightweight viewer…";
    error = null;
    selectedOfficePlugin = null;
    officePluginNotice = "";
    try {
      const nextDoc = await openFile(materializeExternalUri(uri, officeExt()));
      if (seq !== loadSeq || (docUri ?? pendingUri) !== uri) return;
      doc = nextDoc;
      officePluginNotice = "Rendered by built-in lightweight viewer.";
      saveRuntimePref(officeExt(), "__builtin__");
    } catch (e) {
      if (seq !== loadSeq || (docUri ?? pendingUri) !== uri) return;
      error = e instanceof Error ? e.message : String(e);
      doc = null;
    } finally {
      if (seq === loadSeq && (docUri ?? pendingUri) === uri) {
        busy = false;
        busyHint = "";
      }
    }
  }
  // Drag-and-drop open (web + desktop webview, uniform).
  let dragOver = $state(false);
  let dragDepth = 0;
  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
  }
  function handleDragEnter() {
    dragDepth++;
    dragOver = true;
  }
  function handleDragLeave() {
    dragDepth = Math.max(0, dragDepth - 1);
    if (dragDepth === 0) dragOver = false;
  }
  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    dragDepth = 0;
    dragOver = false;
    const file = e.dataTransfer?.files?.[0];
    if (!file) return;
    mode = "view";
    await pickFile(file);
  }
</script>

<div
  class="viewit-root"
  class:dragover={dragOver}
  class:fs-mode={fullscreenState.active}
  data-root={root}
  role="region"
  aria-label="ViewIt file viewer"
  aria-busy={busy}
  ondragover={handleDragOver}
  ondragenter={handleDragEnter}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
>
  <header class="app-header" bind:this={headerEl}>
    <div class="brand">
      <h1>ViewIt</h1>
      <span>Universal file viewer</span>
    </div>
    <div class="header-actions">
      <button
        class="theme-toggle"
        onclick={toggleTheme}
        aria-label="Toggle dark mode"
        title="Toggle theme"
      >
        <Icon name={theme.mode === "dark" ? "sun" : "moon"} />
      </button>
      {#if root === "mobile"}
        <button
          class="plugin-btn"
          onclick={() => (pluginStoreOpen = true)}
          aria-label="Plugin store"
          title="Plugin store"
        >
          <Icon name="package" />
        </button>
      {/if}
      <button class="primary-action" onclick={pick}>
        <Icon name="file-plus" />
        <span>Open file</span>
      </button>
      <button
        type="button"
        class="hint-btn"
        title="Settings & Preferences"
        aria-label="Open settings"
        onclick={() => (settingsOpen = true)}
      >
        <Icon name="sliders" />
      </button>
      <button
        type="button"
        class="hint-btn"
        title="Tap for on-device debug log (replaces Chrome inspect)"
        aria-label={debugOpen ? "Hide debug log" : "Show debug log"}
        onclick={() => (debugOpen = !debugOpen)}
      >
        <Icon name="terminal" />
        <span>Log</span>
        <Icon name={debugOpen ? "chevron-up" : "chevron-down"} size={14} />
      </button>
      <button
        class="mode-toggle"
        onclick={() => (mode = mode === "view" ? "browse" : "view")}
        aria-label={mode === "view" ? "Browse files" : "Return to viewer"}
        title={mode === "view" ? "Browse files" : "Return to viewer"}
      >
        <Icon name={mode === "view" ? "grid" : "arrow-left"} />
      </button>
    </div>
  </header>

  <main bind:this={mainEl}>
    {#if mode === "browse"}
      <GridView
        {root}
        initialPath={browsePath}
        onListed={onGridListed}
        {upSignal}
        searchQuery={mode === "browse" ? searchQuery : ""}
        onPick={(f) => pickFile(f)}
      />
    {:else if !busy && !error && !doc}
      <Onboarding />
      <p class="empty">Drop a file or pick one — everything opens.</p>
      <GridView
        {root}
        initialPath={browsePath}
        onListed={onGridListed}
        {upSignal}
        {searchQuery}
        onPick={(f) => pickFile(f)}
      />
    {:else if busy}
      <div class="state-card" role="status" aria-live="polite">
        <span class="spinner" aria-hidden="true"></span>
        <p class="status">{busyHint || "Reading file…"}</p>
      </div>
    {:else if error}
      <div class="state-card error-card" role="alert">
        <strong>ViewIt could not open this file</strong>
        <pre class="error">{error}</pre>
        <button type="button" onclick={pick}>Choose another file</button>
      </div>
    {:else if doc}
      {#if formatInstallCta}
        <div class="plugin-cta">
          <span
            >This format is provided by the <strong>{formatInstallCta.pluginName}</strong> plugin.</span
          >
          <button type="button" onclick={confirmFormatInstall} disabled={formatInstalling}>
            {formatInstalling ? "Installing…" : "Install & reopen"}
          </button>
          <button
            type="button"
            class="dismiss"
            onclick={() => (formatInstallCta = null)}
            aria-label="Dismiss plugin suggestion"
            title="Dismiss"><Icon name="x" /></button
          >
        </div>
      {/if}
      {#if ["text", "markdown", "json"].includes(doc.kind)}
        <details class="editor-details">
          <summary>{editPlugin ? `Edit with ${editPlugin.name}` : "Edit with a plugin"}</summary>
          <EditorBaseHost
            text={doc.kind === "text"
              ? ((doc as any).content ?? "")
              : doc.kind === "json"
                ? ((doc as any).pretty ?? "")
                : ((doc as any).html ?? "")}
            name={pendingName ?? docUri?.split("/").pop() ?? "document.txt"}
            mime={doc.kind === "json"
              ? "application/json"
              : doc.kind === "markdown"
                ? "text/markdown"
                : "text/plain"}
            plugin={editPlugin}
          />
        </details>
      {/if}
      {#if doc.kind === "text"}
        {#key docUri}
          {#if docUri && /\.ics?$/i.test(docUri) && IcsViewer}
            <IcsViewer {...doc as any} />
          {:else if docUri && /\.vcf$/i.test(docUri) && VcfViewer}
            <VcfViewer {...doc as any} />
          {:else if docUri && /\.desktop$/i.test(docUri) && DesktopEntryViewer}
            <DesktopEntryViewer {...doc as any} />
          {:else}
            <TextViewer
              {...doc as any}
              {searchQuery}
              {searchCaseSensitive}
              searchNav={searchNavCounter}
              onSearchResult={(count: number, current: number) => {
                searchMatchCount = count;
                searchCurrentMatch = current;
              }}
            />
          {/if}
        {/key}
      {:else if doc.kind === "image" && docUri}
        {#key docUri}
          <ImageViewer uri={docUri} {...doc as any} />
        {/key}
      {:else if doc.kind === "markdown" && MarkdownViewer}
        {#key docUri}
          <MarkdownViewer {...doc as any} />
        {/key}
      {:else if doc.kind === "json" && JsonViewer}
        {#key docUri}
          <JsonViewer {...doc as any} />
        {/key}
      {:else if doc.kind === "csv" && CsvViewer}
        {#key docUri}
          <CsvViewer {...doc as any} />
        {/key}
      {:else if doc.kind === "stream-file"}
        {#key docUri}
          {#if PdfViewer}
            <PdfViewer document={doc} source_uri={docUri ?? ""} />
          {:else}
            <p class="status">Loading PDF viewer…</p>
          {/if}
        {/key}
      {:else if doc.kind === "media"}
        {#key docUri}
          {#if playPlugin}
            <div class="media-cta">
              <span>Player base: <strong>{playPlugin.name}</strong></span>
              <button type="button" onclick={() => (usePlayerBase = !usePlayerBase)}
                >{usePlayerBase ? "Use built-in player" : "Use custom player"}</button
              >
            </div>
          {/if}
          {#if usePlayerBase && playPlugin}
            <PlayerBaseHost
              source={docUri ?? ""}
              stream={(doc as any)?.stream_url ?? ""}
              kind={((doc as any)?.media_kind ?? "video") === "audio" ? "audio" : "video"}
              name={(doc as any)?.name ?? ""}
              plugin={playPlugin}
            />
          {:else}
            <MediaViewer uri={docUri ?? ""} {...doc as any} />
          {/if}
        {/key}
      {:else if doc.kind === "pdf" && PdfViewer}
        {#key docUri}
          <PdfViewer document={doc} source_uri={docUri ?? ""} />
        {/key}
      {:else if (doc.kind === "epub" || doc.kind === "mobi" || doc.kind === "azw3") && EpubViewer}
        {#key docUri}
          <EpubViewer
            {...doc as any}
            uri={docUri ?? ""}
            onclose={() => {
              doc = null;
            }}
          />
        {/key}
      {:else if doc.kind === "archive" && ArchiveViewer}
        {#key docUri}
          <ArchiveViewer
            {...doc as any}
            name={pendingName ?? ""}
            uri={docUri ?? ""}
            onOpenDocument={(nestedDoc: any, nestedUrl: string, nestedName: string) => {
              const scope = replaceDocumentScope(`archive-member:${nestedName}`);
              scope.ownObjectUrl(nestedUrl);
              doc = nestedDoc;
              docUri = nestedUrl;
              pendingName = nestedName;
              pendingExt = extFromUri(nestedName);
            }}
          />
        {/key}
      {:else if isOfficePluginHtml()}
        {#key docUri}
          <div class="plugin-renderer-shell">
            <div class="runtime-bar plugin-runtime-bar">
              <button type="button" onclick={() => (officeRuntimeChooserOpen = true)}
                >Runtime: {selectedOfficePlugin?.name ?? "Office plugin"}</button
              >
              {#if officePluginNotice}<span>{officePluginNotice}</span>{/if}
              {#if officeFidelity}<span class="fidelity">{officeFidelity}</span>{/if}
              {#if officeWarnings.length > 0}<details class="warnings">
                  <summary>{officeWarnings.length} warning(s)</summary>
                  <ul>
                    {#each officeWarnings as w}<li>{w}</li>{/each}
                  </ul>
                </details>{/if}
            </div>
            <OfficePluginHtmlViewer document={doc} />
          </div>
        {/key}
      {:else if doc.kind === "pptx" && PptxViewer}
        {#key docUri}
          <div class:plugin-renderer-shell={isPluginRendered()}>
            {#if isIworkDocument()}
              <div class="runtime-bar plugin-runtime-bar">
                <strong>{iworkLabel()}</strong>
                <span>Best-effort partial preview from the iWork package.</span>
              </div>
            {:else}
              <div class="runtime-bar" class:plugin-runtime-bar={isPluginRendered()}>
                <button type="button" onclick={() => (officeRuntimeChooserOpen = true)}
                  >Runtime: {selectedOfficePlugin?.name ?? "Built-in lightweight viewer"}</button
                >
                {#if officePluginNotice}<span>{officePluginNotice}</span>{/if}
                {#if officeFidelity}<span class="fidelity">{officeFidelity}</span>{/if}
                {#if officeWarnings.length > 0}<details class="warnings">
                    <summary>{officeWarnings.length} warning(s)</summary>
                    <ul>
                      {#each officeWarnings as w}<li>{w}</li>{/each}
                    </ul>
                  </details>{/if}
              </div>
            {/if}
            {#if !isIworkDocument() && PptxVanillaViewer && pptxVanillaPlugin}
              <PptxVanillaViewer
                document={doc}
                source_uri={docUri ?? ""}
                plugin={pptxVanillaPlugin}
              />
            {:else if (doc as any).html}<div class="plugin-html-surface">
                {@html (doc as any).html}
              </div>{:else}<PptxViewer
                document={doc}
                source_uri={docUri ?? ""}
                source_kind={isIworkDocument() ? "iwork" : "office"}
              />{/if}
          </div>
        {/key}
      {:else if doc.kind === "docx" && (officeExt() === "odt" || officeExt() === "ott" || isIworkDocument() || Array.isArray((doc as any).blocks) ? DocxViewer : DocxPreview)}
        {#key docUri}
          <div class="plugin-renderer-shell">
            {#if isIworkDocument()}
              <div class="runtime-bar plugin-runtime-bar">
                <strong>{iworkLabel()}</strong>
                <span>Best-effort partial preview from the iWork package.</span>
              </div>
            {:else}
              <div class="runtime-bar plugin-runtime-bar">
                <button type="button" onclick={() => (officeRuntimeChooserOpen = true)}
                  >Runtime: {selectedOfficePlugin?.name ?? "Office Universal"}</button
                >
                {#if officePluginNotice}<span>{officePluginNotice}</span>{/if}
                {#if officeFidelity}<span class="fidelity">{officeFidelity}</span>{/if}
                {#if officeWarnings.length > 0}<details class="warnings">
                    <summary>{officeWarnings.length} warning(s)</summary>
                    <ul>
                      {#each officeWarnings as w}<li>{w}</li>{/each}
                    </ul>
                  </details>{/if}
              </div>
            {/if}
            {#if officeExt() === "odt" || officeExt() === "ott" || isIworkDocument() || Array.isArray((doc as any).blocks)}
              <DocxViewer
                document={doc}
                {searchQuery}
                {searchCaseSensitive}
                searchNav={searchNavCounter}
                onSearchResult={(count: number, current: number) => {
                  searchMatchCount = count;
                  searchCurrentMatch = current;
                }}
              />
            {:else}
              <DocxPreview source_uri={docUri ?? ""} />
            {/if}
          </div>
        {/key}
      {:else if doc.kind === "xlsx" && XlsxViewer}
        {#key docUri}
          <div class:plugin-renderer-shell={isPluginRendered()}>
            {#if isIworkDocument()}
              <div class="runtime-bar plugin-runtime-bar">
                <strong>{iworkLabel()}</strong>
                <span>Best-effort partial preview from the iWork package.</span>
              </div>
            {:else}
              <div class="runtime-bar" class:plugin-runtime-bar={isPluginRendered()}>
                <button type="button" onclick={() => (officeRuntimeChooserOpen = true)}
                  >Runtime: {selectedOfficePlugin?.name ?? "Built-in lightweight viewer"}</button
                >
                {#if officePluginNotice}<span>{officePluginNotice}</span>{/if}
                {#if officeFidelity}<span class="fidelity">{officeFidelity}</span>{/if}
                {#if officeWarnings.length > 0}<details class="warnings">
                    <summary>{officeWarnings.length} warning(s)</summary>
                    <ul>
                      {#each officeWarnings as w}<li>{w}</li>{/each}
                    </ul>
                  </details>{/if}
              </div>
            {/if}
            {#if (doc as any).html}<div class="plugin-html-surface">
                {@html (doc as any).html}
              </div>{:else}<XlsxViewer
                document={doc}
                {searchQuery}
                {searchCaseSensitive}
                searchNav={searchNavCounter}
                onSearchResult={(count: number, current: number) => {
                  searchMatchCount = count;
                  searchCurrentMatch = current;
                }}
              />{/if}
          </div>
        {/key}
      {:else if doc.kind === "unsupported"}
        {#key docUri}
          <UnsupportedViewer
            uri={docUri ?? undefined}
            document={{
              ...(doc as any),
              format:
                (doc as any).format === "unsupported"
                  ? pendingExt ||
                    extFromUri(docUri ?? "", pendingName ?? undefined) ||
                    "unsupported"
                  : (doc as any).format,
            }}
            onPluginInstalled={() => {
              if (docUri) {
                pendingUri = docUri;
                void load();
              }
            }}
          />
        {/key}
      {:else if doc.kind === "font" && FontViewer}
        {#key docUri}
          <FontViewer {...doc as any} />
        {/key}
      {:else if doc.kind === "placeholder"}
        {#key docUri}
          <PlaceholderViewer document={doc as any} onInstall={() => (pluginStoreOpen = true)} />
        {/key}
      {:else}
        <p class="error">Unknown document kind: <code>{(doc as any).kind}</code></p>
      {/if}
    {:else}
      <p class="empty">Drop a file or pick one — everything opens.</p>
    {/if}
  </main>
  <footer class="bottom-bar">
    <SearchBar
      onSearch={onGlobalSearch}
      matchCount={searchMatchCount}
      currentMatch={searchCurrentMatch}
      onNext={() => searchNavCounter++}
      onPrev={() => searchNavCounter--}
    />
    {#if doc && !busy}
      <button
        type="button"
        class="fs-toggle"
        onclick={() => setFullscreen(!fullscreenState.active)}
        aria-label={fullscreenState.active ? "Exit full screen" : "Enter full screen"}
        aria-pressed={fullscreenState.active}
        title={fullscreenState.active ? "Exit full screen" : "Full screen"}
      >
        <Icon name={fullscreenState.active ? "minimize" : "maximize"} />
        <span>{fullscreenState.active ? "Exit" : "Expand"}</span>
      </button>
    {/if}
  </footer>
  <DebugPanel bind:open={debugOpen} />
  <PluginStore open={pluginStoreOpen} onClose={() => (pluginStoreOpen = false)} />
  <SettingsModal bind:open={settingsOpen} onClose={() => (settingsOpen = false)} />
  <RuntimeChooser
    open={officeRuntimeChooserOpen}
    uri={docUri ?? ""}
    name={(docUri ?? "Office file").split("/").pop() ?? "Office file"}
    ext={officeExt()}
    builtInLabel="Built-in lightweight Office viewer"
    service="viewit.document.parse"
    onClose={() => (officeRuntimeChooserOpen = false)}
    onUseBuiltIn={chooseBuiltInOfficeRuntime}
    onUseInstalledPlugin={chooseOfficePlugin}
  />
</div>

<style>
  :global(*) {
    box-sizing: border-box;
  }
  :global(:root) {
    --header-h: 3.5rem;
    --text-primary: #222;
    --text-secondary: #888;
    --bg-primary: #fff;
    --bg-secondary: #f7f7f7;
    --border: #ddd;
    --error: #b22;
    --link: #124fc2;
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
  :global(html),
  :global(body) {
    margin: 0;
    height: 100%;
    overflow: hidden;
    overscroll-behavior: none;
    -webkit-text-size-adjust: 100%;
    text-size-adjust: 100%;
  }
  :global(body) {
    font-family: system-ui, sans-serif;
    color: var(--text-primary);
    background: var(--bg-primary);
  }
  /* Hard-frozen app chrome: the shell is exactly the viewport and never grows
     with document content. `main` is the only scroll container, so a file's
     size or width can never reflow, wrap, or scale the header/controls
     (this also prevents Android WebView zoom-out-to-fit on wide content). */
  .viewit-root {
    display: flex;
    flex-direction: column;
    height: 100vh;
    height: 100dvh;
    overflow: hidden;
    transition: outline-color 0.15s ease;
  }
  .viewit-root.dragover {
    outline: 3px dashed var(--link);
    outline-offset: -8px;
  }
  .app-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: calc(0.65rem + env(safe-area-inset-top, 0px))
      max(1rem, env(safe-area-inset-right, 0px)) 0.65rem max(1rem, env(safe-area-inset-left, 0px));
    border-bottom: 1px solid var(--border);
    background: var(--bg-primary);
    flex-shrink: 0;
    z-index: 10;
  }
  .brand {
    min-width: 0;
  }
  .brand h1 {
    font-size: 1.2rem;
    margin: 0;
    letter-spacing: -0.01em;
    color: var(--text-primary);
    white-space: nowrap;
  }
  .brand span {
    display: block;
    margin-top: 0.08rem;
    color: var(--text-secondary);
    font-size: 0.7rem;
    letter-spacing: 0.02em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .header-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: nowrap;
    justify-content: flex-end;
    flex-shrink: 0;
  }
  .hint-btn {
    padding: 0.25rem 0.55rem;
    min-width: 44px;
    min-height: 44px;
  }
  .app-header button {
    cursor: pointer;
    background: var(--bg-secondary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    padding: 0.3rem 0.7rem;
    font-size: 0.85rem;
    min-width: 44px;
    min-height: 44px;
    flex-shrink: 0;
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
  }
  .app-header button:hover {
    background: var(--border);
  }
  .app-header .primary-action {
    background: var(--text-primary);
    border-color: var(--text-primary);
    color: var(--bg-primary);
    font-weight: 650;
  }
  .theme-toggle {
    font-size: 1rem;
    padding: 0.3rem 0.5rem;
  }
  .plugin-btn {
    font-size: 1rem;
    padding: 0.3rem 0.5rem;
  }
  .mode-toggle {
    font-size: 1rem;
    padding: 0.3rem 0.5rem;
  }
  .fs-toggle {
    font-size: 1rem;
    padding: 0.3rem 0.5rem;
  }
  .fs-toggle.pending {
    visibility: hidden;
  }
  .fs-toggle[aria-pressed="true"] {
    background: var(--border);
  }
  /* Full screen mode: chrome steps aside; each handler restyles its own
     interior via `fullscreenState`. The chip guarantees a visible exit. */
  .bottom-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    padding: 0.5rem max(1rem, env(safe-area-inset-right, 0px))
      calc(0.5rem + env(safe-area-inset-bottom, 0px)) max(1rem, env(safe-area-inset-left, 0px));
    border-top: 1px solid var(--border);
    background: var(--bg-primary);
    flex-shrink: 0;
    z-index: 10;
  }
  .bottom-bar .fs-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 120px;
    justify-content: center;
  }
  .viewit-root.fs-mode .app-header,
  .viewit-root.fs-mode .bottom-bar {
    display: none;
  }
  .viewit-root.fs-mode main {
    padding: 0;
    max-width: none;
  }
  .viewit-root.fs-mode {
    background: var(--bg-primary);
  }
  .fs-exit-chip {
    position: fixed;
    top: max(0.6rem, env(safe-area-inset-top, 0px));
    right: max(0.6rem, env(safe-area-inset-right, 0px));
    z-index: 60;
    width: 44px;
    height: 44px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: color-mix(in srgb, var(--bg-primary) 82%, transparent);
    color: var(--text-primary);
    cursor: pointer;
    backdrop-filter: blur(6px);
    opacity: 0.55;
    transition: opacity 0.15s ease;
  }
  .fs-exit-chip:hover,
  .fs-exit-chip:focus-visible {
    opacity: 1;
  }
  main {
    padding: 1rem max(1rem, env(safe-area-inset-right, 0px))
      max(1rem, env(safe-area-inset-bottom, 0px)) max(1rem, env(safe-area-inset-left, 0px));
    flex: 1;
    min-height: 0;
    width: 100%;
    overflow-y: auto;
    overflow-x: auto;
    overscroll-behavior: contain;
  }
  .state-card {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: min(100%, 34rem);
    margin: clamp(2rem, 12vh, 8rem) auto 0;
    padding: 1rem 1.1rem;
    border: 1px solid var(--border);
    border-radius: 0.8rem;
    background: var(--bg-secondary);
  }
  .state-card p,
  .state-card pre {
    margin: 0;
  }
  .state-card.error-card {
    align-items: stretch;
    flex-direction: column;
  }
  .state-card button {
    align-self: flex-start;
    min-height: 44px;
    padding: 0.45rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 0.45rem;
    background: var(--bg-primary);
    color: var(--text-primary);
    cursor: pointer;
  }
  .spinner {
    width: 1.15rem;
    height: 1.15rem;
    flex: 0 0 auto;
    border: 2px solid var(--border);
    border-top-color: var(--link);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .status,
  .empty {
    color: var(--text-secondary);
    font-style: italic;
  }
  .error {
    color: var(--error);
    white-space: pre-wrap;
  }
  .runtime-bar {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    flex-wrap: wrap;
    margin: 0 0 0.75rem;
    color: var(--text-secondary);
    font-size: 0.8rem;
  }
  .runtime-bar button {
    cursor: pointer;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--bg-secondary);
    color: var(--text-primary);
    padding: 0.35rem 0.7rem;
  }
  .runtime-bar span {
    flex: 1 1 18rem;
  }
  .runtime-bar .fidelity {
    font-style: italic;
    opacity: 0.8;
    font-size: 0.72rem;
  }
  .runtime-bar .warnings {
    font-size: 0.72rem;
    color: var(--text-secondary);
  }
  .runtime-bar .warnings summary {
    cursor: pointer;
  }
  .runtime-bar .warnings ul {
    margin: 0.25rem 0 0;
    padding-left: 1rem;
    max-width: 40rem;
  }
  .plugin-renderer-shell {
    display: grid;
    gap: 0.75rem;
  }
  .plugin-runtime-bar {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.4rem 0.55rem;
  }
  .plugin-html-surface {
    overflow: auto;
    border-radius: 0.75rem;
  }
  .plugin-cta {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
    margin-bottom: 0.6rem;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--link);
    border-radius: 0.6rem;
    background: color-mix(in srgb, var(--link) 12%, transparent);
    color: var(--text-primary);
    font-size: 0.86rem;
  }
  .plugin-cta strong {
    color: var(--link);
  }
  .plugin-cta button {
    padding: 0.35rem 0.7rem;
    border: 1px solid var(--link);
    border-radius: 0.4rem;
    background: var(--link);
    color: #fff;
    cursor: pointer;
    font-weight: 600;
  }
  .plugin-cta button:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .plugin-cta button.dismiss {
    margin-left: auto;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 1.1rem;
    line-height: 1;
  }
  .media-cta {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
    margin-bottom: 0.6rem;
    padding: 0.4rem 0.7rem;
    border: 1px solid var(--link);
    border-radius: 0.6rem;
    background: color-mix(in srgb, var(--link) 10%, transparent);
    font-size: 0.85rem;
    color: var(--text-primary);
  }
  .media-cta strong {
    color: var(--link);
  }
  .media-cta button {
    padding: 0.3rem 0.6rem;
    border: 1px solid var(--link);
    border-radius: 0.4rem;
    background: var(--link);
    color: #fff;
    cursor: pointer;
    font-weight: 600;
  }
  .editor-details {
    margin-bottom: 0.55rem;
    border: 1px solid var(--link);
    border-radius: 0.6rem;
    padding: 0.4rem 0.7rem;
    background: color-mix(in srgb, var(--link) 10%, transparent);
    color: var(--text-primary);
  }
  .editor-details summary {
    cursor: pointer;
    color: var(--link);
    font-size: 0.85rem;
    font-weight: 700;
  }
  .editor-details[open] summary {
    margin-bottom: 0.6rem;
  }
  :global(button:focus-visible),
  :global(a:focus-visible),
  :global(input:focus-visible),
  :global(select:focus-visible),
  :global(summary:focus-visible),
  :global([tabindex]:focus-visible) {
    outline: 3px solid color-mix(in srgb, var(--link) 70%, white);
    outline-offset: 2px;
  }
  @media (max-width: 680px) {
    .app-header {
      align-items: center;
      gap: 0.55rem;
    }
    .brand {
      flex: 0 0 auto;
    }
    .brand span {
      display: none;
    }
    .brand h1 {
      font-size: 1rem;
    }
    /* One row always — actions scroll horizontally instead of wrapping, so
       opening a file can never shift or reflow any control. */
    .header-actions {
      overflow-x: auto;
      scrollbar-width: none;
      -webkit-overflow-scrolling: touch;
    }
    .header-actions::-webkit-scrollbar {
      display: none;
    }
    .hint-btn span {
      display: none;
    }
    main {
      padding-top: 0.75rem;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .viewit-root {
      transition: none;
    }
    .spinner {
      animation-duration: 1.6s;
    }
  }
</style>
