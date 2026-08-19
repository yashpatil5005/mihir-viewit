<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    title = "",
    author = null,
    first_chapter_xhtml = "",
    spine_len = 0,
    byte_len = 0,
    uri = "",
    onclose = null,
  }: {
    title: string;
    author?: string | null;
    first_chapter_xhtml: string;
    spine_len: number;
    byte_len: number;
    uri?: string;
    onclose?: (() => void) | null;
  } = $props();

  let currentChapter = $state(0);
  let currentXhtml = $state("");
  let loading = $state(false);

  function styledXhtml(html: string): string {
    if (!html) return "";
    const sanitized = html.replace(/<link\b[^>]*\brel=["']stylesheet["'][^>]*>/gi, "");
    const style = `<style>
       * { box-sizing: border-box; }
       html { min-height: 100%; overflow-x: hidden; overflow-y: auto; background: #0d1117; color-scheme: dark; }
       body { min-height: 100%; max-width: 46rem; margin: 0 auto; padding: clamp(1.25rem, 5vw, 3.5rem); background: #0d1117; color: #e6edf3; font-family: Georgia, 'Times New Roman', serif; line-height: 1.8; font-size: clamp(1rem, 0.96rem + 0.3vw, 1.2rem); overflow-wrap: anywhere; -webkit-text-size-adjust: 100%; }
       img { max-width: 100%; max-height: 70vh; object-fit: contain; margin: 1rem 0; }
       h1, h2, h3, h4 { margin: 1rem 0 0.5rem; color: #e6edf3; }
       p { margin: 0.6rem 0; }
       a { color: #79b8ff; text-underline-offset: 0.18em; }
       a:focus-visible { outline: 3px solid #79b8ff; outline-offset: 3px; border-radius: 2px; }
       table { border-collapse: collapse; display: block; width: 100%; margin: 1rem 0; overflow-x: auto; }
       td, th { border: 1px solid #30363d; padding: 0.4rem 0.6rem; text-align: left; }
       ::selection { background: #1f6feb; color: white; }
    </style>`;
    if (sanitized.includes("<head>")) return sanitized.replace("<head>", `<head>${style}`);
    if (sanitized.includes("<html>")) return sanitized.replace("<html>", `<html>${style}`);
    return style + sanitized;
  }

  async function loadChapter(idx: number) {
    if (idx < 0 || idx >= spine_len || loading) return;
    if (idx === 0) {
      currentChapter = 0;
      currentXhtml = first_chapter_xhtml;
      return;
    }
    if (typeof (window as any).__TAURI_INTERNALS__ === "undefined") return;
    try {
      const invoke =
        (window as any).__TAURI_INTERNALS__.invoke ?? (await import("@tauri-apps/api/core")).invoke;
      loading = true;
      const html = await invoke("epub_chapter", { uri, index: idx });
      currentXhtml = html;
      currentChapter = idx;
    } catch (e) {
      console.error("epub_chapter failed:", e);
    } finally {
      loading = false;
    }
  }

  function prevChapter() {
    if (currentChapter > 0) loadChapter(currentChapter - 1);
  }
  function nextChapter() {
    if (currentChapter < spine_len - 1) loadChapter(currentChapter + 1);
  }

  let showToc = $state(false);

  function toggleToc() {
    showToc = !showToc;
  }

  function selectChapter(idx: number) {
    showToc = false;
    loadChapter(idx);
  }

  function goBack() {
    if (onclose) onclose();
    else if (typeof (window as any).__TAURI_INTERNALS__ !== "undefined") {
      history.back();
    }
  }

  $effect(() => {
    if (currentChapter === 0) currentXhtml = first_chapter_xhtml;
  });

  function handleReaderKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      goBack();
    }
  }
</script>

<svelte:window onkeydown={handleReaderKeydown} />

<div class="epub-fullscreen">
  {#if loading}
    <div class="loading-overlay">Loading…</div>
  {/if}

  <header class="reader-toolbar">
    <button class="back-btn" onclick={goBack} aria-label="Close EPUB"
      ><Icon name="arrow-left" /></button
    >
    <div class="book-meta">
      <strong>{title || "Untitled book"}</strong>
      {#if author}<span>{author}</span>{/if}
    </div>
    <nav class="reader-nav" aria-label="Book chapters">
      <button onclick={prevChapter} disabled={currentChapter === 0} aria-label="Previous chapter"
        ><Icon name="arrow-left" size={17} /></button
      >
      <button
        class="toc-toggle-btn"
        class:active={showToc}
        onclick={toggleToc}
        aria-label="Table of contents"
        aria-expanded={showToc}
      >
        <span>{currentChapter + 1} / {spine_len}</span><Icon name="list" size={17} />
      </button>
      <button
        onclick={nextChapter}
        disabled={currentChapter >= spine_len - 1}
        aria-label="Next chapter"><Icon name="arrow-right" size={17} /></button
      >
    </nav>
    <div class="reader-progress" aria-hidden="true">
      <div
        class="progress-bar"
        style="width: {spine_len ? ((currentChapter + 1) / spine_len) * 100 : 0}%"
      ></div>
    </div>
    {#if showToc}
      <div class="toc-popover">
        <div class="toc-header">Table of contents</div>
        <div class="toc-list">
          {#each Array.from({ length: spine_len }, (_, i) => i) as idx}
            <button
              class="toc-item"
              class:active={idx === currentChapter}
              onclick={() => selectChapter(idx)}
              aria-current={idx === currentChapter ? "page" : undefined}
            >
              <span>Chapter {idx + 1}</span><small>{idx + 1} / {spine_len}</small>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </header>

  <main class="frame-wrapper">
    <iframe
      srcdoc={styledXhtml(currentXhtml)}
      sandbox="allow-same-origin"
      title="{title} chapter {currentChapter + 1}"
      class="epub-frame"
    ></iframe>
  </main>
</div>

<style>
  .epub-fullscreen {
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    height: 100dvh;
    background: #0d1117;
    overflow: hidden;
    z-index: 1000;
    display: flex;
    flex-direction: column;
  }
  .frame-wrapper {
    position: relative;
    flex: 1;
    min-height: 0;
    width: 100%;
  }
  .epub-frame {
    width: 100%;
    height: 100%;
    border: none;
    background: #0d1117;
  }
  .loading-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(13, 17, 23, 0.85);
    color: #8b949e;
    font-size: 1rem;
    z-index: 10;
    pointer-events: none;
  }
  .reader-toolbar {
    position: relative;
    flex: 0 0 auto;
    z-index: 20;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.75rem;
    padding: max(0.55rem, env(safe-area-inset-top, 0px))
      max(0.75rem, env(safe-area-inset-right, 0px)) 0.55rem
      max(0.75rem, env(safe-area-inset-left, 0px));
    background: rgba(13, 17, 23, 0.92);
    border-bottom: 1px solid #30363d;
    backdrop-filter: blur(18px) saturate(140%);
  }
  .back-btn {
    background: rgba(255, 255, 255, 0.1);
    color: #e6edf3;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 0.375rem;
    min-width: 44px;
    min-height: 44px;
    display: grid;
    place-items: center;
    cursor: pointer;
    flex-shrink: 0;
  }
  .back-btn:hover {
    background: rgba(255, 255, 255, 0.2);
  }
  .book-meta {
    min-width: 0;
  }
  .book-meta strong,
  .book-meta span {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .book-meta strong {
    font-size: 0.9rem;
    font-weight: 600;
    color: #e6edf3;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .book-meta span {
    font-size: 0.75rem;
    color: #8b949e;
  }
  .reader-nav {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .reader-nav button {
    min-width: 44px;
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    padding: 0.4rem 0.6rem;
    background: rgba(255, 255, 255, 0.1);
    color: #e6edf3;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 0.375rem;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .reader-nav button:hover:not(:disabled),
  .reader-nav button.active {
    background: rgba(255, 255, 255, 0.2);
  }
  .reader-nav button:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }
  .reader-progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    height: 2px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 1px;
    overflow: hidden;
  }
  .progress-bar {
    height: 100%;
    background: #58a6ff;
    border-radius: 1px;
    transition: width 0.3s ease;
  }
  .toc-toggle-btn {
    min-width: 5rem !important;
  }
  .toc-popover {
    position: absolute;
    top: calc(100% + 0.5rem);
    right: max(0.75rem, env(safe-area-inset-right, 0px));
    width: min(22rem, calc(100vw - 1.5rem));
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 6px;
    max-height: 55vh;
    max-height: 55dvh;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    padding: 0.5rem;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    pointer-events: auto;
  }
  .toc-header {
    font-size: 0.85rem;
    font-weight: 600;
    color: #8b949e;
    margin-bottom: 0.4rem;
    padding: 0.3rem 0.5rem 0.55rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .toc-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .toc-item {
    text-align: left;
    background: transparent !important;
    border: none !important;
    color: #c9d1d9 !important;
    padding: 0.4rem 0.6rem !important;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.9rem !important;
    min-height: 44px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }
  .toc-item:hover {
    background: #21262d !important;
    color: #58a6ff !important;
  }
  .toc-item.active {
    background: #1f6feb !important;
    color: #ffffff !important;
    font-weight: 600;
  }
  .toc-item small {
    color: inherit;
    opacity: 0.7;
  }
  @media (max-width: 520px) {
    .reader-toolbar {
      grid-template-columns: auto minmax(0, 1fr);
    }
    .reader-nav {
      grid-column: 1 / -1;
      justify-content: space-between;
    }
    .reader-nav button:not(.toc-toggle-btn) {
      flex: 1;
    }
    .reader-nav .toc-toggle-btn {
      flex: 2;
    }
  }
  @media (prefers-reduced-transparency: reduce) {
    .reader-toolbar {
      background: #0d1117;
      backdrop-filter: none;
    }
  }
</style>
