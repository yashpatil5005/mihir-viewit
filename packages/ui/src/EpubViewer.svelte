<script lang="ts">
  import { onMount, onDestroy } from 'svelte';

  let {
    title = '',
    author = null,
    first_chapter_xhtml = '',
    spine_len = 0,
    byte_len = 0,
    uri = '',
    onclose = null
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
  let currentXhtml = $state('');
  let loading = $state(false);
  let showOverlay = $state(true);
  let scale = $state(1);

  let overlayTimer: ReturnType<typeof setTimeout> | null = null;
  let touchLayerEl: HTMLDivElement;

  function styledXhtml(html: string): string {
    if (!html) return '';
    const style = `<style>
      * { margin: 0; padding: 0; box-sizing: border-box; }
      html, body { height: 100%; overflow: hidden; background: #0d1117; color: #e6edf3; font-family: Georgia, 'Times New Roman', serif; -webkit-text-size-adjust: none; }
      body { display: flex; flex-direction: column; justify-content: flex-start; padding: 1.5rem; padding-top: 3rem; line-height: 1.8; font-size: 1.1rem; }
      img { max-width: 100%; max-height: 70vh; object-fit: contain; margin: 1rem 0; }
      h1, h2, h3, h4 { margin: 1rem 0 0.5rem; color: #e6edf3; }
      p { margin: 0.6rem 0; }
      a { color: #58a6ff; }
      table { border-collapse: collapse; width: 100%; margin: 1rem 0; }
      td, th { border: 1px solid #30363d; padding: 0.4rem 0.6rem; text-align: left; }
    </style>`;
    if (html.includes('<head>')) return html.replace('<head>', `<head>${style}`);
    if (html.includes('<html>')) return html.replace('<html>', `<html>${style}`);
    return style + html;
  }

  async function loadChapter(idx: number) {
    if (idx < 0 || idx >= spine_len || loading) return;
    if (idx === 0) {
      currentChapter = 0;
      currentXhtml = first_chapter_xhtml;
      return;
    }
    if (typeof (window as any).__TAURI_INTERNALS__ === 'undefined') return;
    try {
      const invoke = (window as any).__TAURI_INTERNALS__.invoke ?? (await import('@tauri-apps/api/core')).invoke;
      loading = true;
      const html = await invoke('epub_chapter', { uri, index: idx });
      currentXhtml = html;
      currentChapter = idx;
    } catch (e) {
      console.error('epub_chapter failed:', e);
    } finally {
      loading = false;
    }
  }

  function prevChapter() { if (currentChapter > 0) loadChapter(currentChapter - 1); }
  function nextChapter() { if (currentChapter < spine_len - 1) loadChapter(currentChapter + 1); }

  function resetOverlayTimer() {
    if (overlayTimer) clearTimeout(overlayTimer);
    overlayTimer = setTimeout(() => { showOverlay = false; }, 3500);
  }

  function goBack() {
    if (onclose) onclose();
    else if (typeof (window as any).__TAURI_INTERNALS__ !== 'undefined') {
      history.back();
    }
  }

  $effect(() => {
    if (currentChapter === 0) currentXhtml = first_chapter_xhtml;
  });

  onMount(() => {
    resetOverlayTimer();

    let ptrStartX = 0;
    let ptrStartY = 0;
    let ptrStartTime = 0;

    function onPtrDown(e: PointerEvent) {
      if (e.isPrimary) {
        ptrStartX = e.clientX;
        ptrStartY = e.clientY;
        ptrStartTime = Date.now();
      }
    }

    function onPtrUp(e: PointerEvent) {
      if (!e.isPrimary) return;
      const dx = e.clientX - ptrStartX;
      const dy = e.clientY - ptrStartY;
      const dt = Date.now() - ptrStartTime;
      if (Math.abs(dx) < 25 && Math.abs(dy) < 25 && dt < 350) {
        const x = e.clientX;
        const w = window.innerWidth;
        if (x < w * 0.15) { prevChapter(); return; }
        if (x > w * 0.85) { nextChapter(); return; }
        showOverlay = !showOverlay;
        if (showOverlay) resetOverlayTimer();
        return;
      }
      if (Math.abs(dx) > 60 && Math.abs(dx) > Math.abs(dy) * 1.2) {
        if (dx < 0) nextChapter(); else prevChapter();
      }
    }

    const el = touchLayerEl;
    if (el) {
      el.addEventListener('pointerdown', onPtrDown);
      el.addEventListener('pointerup', onPtrUp);
    }

    return () => {
      if (el) {
        el.removeEventListener('pointerdown', onPtrDown);
        el.removeEventListener('pointerup', onPtrUp);
      }
    };
  });

  onDestroy(() => {
    if (overlayTimer) clearTimeout(overlayTimer);
  });
</script>

<div class="epub-fullscreen">
  {#if loading}
    <div class="loading-overlay">Loading…</div>
  {/if}

  {#if showOverlay}
    <div class="overlay">
      <div class="overlay-header">
        <button class="back-btn" onclick={goBack} aria-label="Close EPUB">←</button>
        <div class="overlay-title">{title}</div>
      </div>
      {#if author}<div class="overlay-author">{author}</div>{/if}
      <div class="overlay-nav">
        <button onclick={prevChapter} disabled={currentChapter === 0}>← Prev</button>
        <span class="overlay-chapter">{currentChapter + 1} / {spine_len}</span>
        <button onclick={nextChapter} disabled={currentChapter >= spine_len - 1}>Next →</button>
      </div>
      <div class="overlay-progress">
        <div class="progress-bar" style="width: {((currentChapter + 1) / spine_len) * 100}%"></div>
      </div>
    </div>
  {/if}

  <div class="touch-layer" bind:this={touchLayerEl}></div>

  <div class="frame-wrapper" style="transform: scale({scale}); transform-origin: center center;">
    <iframe
      srcdoc={styledXhtml(currentXhtml)}
      sandbox="allow-same-origin"
      title="{title} chapter {currentChapter + 1}"
      class="epub-frame"
    ></iframe>
  </div>
</div>

<style>
  .epub-fullscreen {
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    background: #0d1117;
    overflow: hidden;
    z-index: 1000;
  }
  .frame-wrapper {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    transition: transform 0.15s ease;
  }
  .epub-frame {
    width: 100%;
    height: 100%;
    border: none;
    background: #0d1117;
  }
  .touch-layer {
    position: absolute;
    inset: 0;
    z-index: 5;
    touch-action: none;
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
  .overlay {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 20;
    pointer-events: none;
    background: linear-gradient(180deg, rgba(13, 17, 23, 0.97) 0%, rgba(13, 17, 23, 0.8) 70%, transparent 100%);
    padding-top: max(env(safe-area-inset-top, 24px), 24px);
    padding-left: 1rem;
    padding-right: 1rem;
    padding-bottom: 2rem;
    animation: fadeIn 0.2s ease;
  }
  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(-10px); }
    to { opacity: 1; transform: translateY(0); }
  }
  .overlay-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.25rem;
  }
  .back-btn {
    background: rgba(255, 255, 255, 0.1);
    color: #e6edf3;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 0.375rem;
    padding: 0.3rem 0.7rem;
    font-size: 0.9rem;
    cursor: pointer;
    flex-shrink: 0;
    pointer-events: auto;
  }
  .back-btn:hover { background: rgba(255, 255, 255, 0.2); }
  .overlay-title {
    font-size: 0.9rem;
    font-weight: 600;
    color: #e6edf3;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .overlay-author {
    font-size: 0.75rem;
    color: #8b949e;
    margin-bottom: 0.5rem;
  }
  .overlay-nav {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 1rem;
  }
  .overlay-nav button {
    padding: 0.4rem 1rem;
    background: rgba(255, 255, 255, 0.1);
    color: #e6edf3;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 0.375rem;
    font-size: 0.8rem;
    cursor: pointer;
    transition: background 0.15s;
    pointer-events: auto;
  }
  .overlay-nav button:hover:not(:disabled) { background: rgba(255, 255, 255, 0.2); }
  .overlay-nav button:disabled { opacity: 0.3; cursor: not-allowed; }
  .overlay-chapter { font-size: 0.8rem; color: #8b949e; }
  .overlay-progress {
    margin-top: 0.75rem;
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
</style>
