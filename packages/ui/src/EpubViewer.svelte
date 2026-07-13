<script lang="ts">
  // Phase 2.8 — EPUB viewer with chapter nav (prev/next + index buttons).
  // Sandboxes chapter XHTML in an iframe srcdoc.

  let {
    title = '',
    author = null,
    first_chapter_xhtml = '',
    spine_len = 0,
    byte_len = 0
  }: {
    title: string;
    author?: string | null;
    first_chapter_xhtml: string;
    spine_len: number;
    byte_len: number;
  } = $props();

  let currentChapter = $state(0);
  let currentXhtml = $state(first_chapter_xhtml);
  let loading = $state(false);

  async function loadChapter(idx: number) {
    if (idx < 0 || idx >= spine_len) return;
    if (idx === 0) {
      currentChapter = 0;
      currentXhtml = first_chapter_xhtml;
      return;
    }
    // Phase 2.8 — fetch via Tauri `epub_chapter` command (desktop only).
    // Web/WASM fallback: no chapter nav beyond first.
    if (typeof (window as any).__TAURI_INTERNALS__ === 'undefined') return;
    try {
      const invoke = (window as any).__TAURI_INTERNALS__.invoke ?? (await import('@tauri-apps/api/core')).invoke;
      loading = true;
      const html = await invoke('epub_chapter', { uri: (window as any).__viewit_uri__, index: idx });
      currentXhtml = html;
      currentChapter = idx;
    } catch (e) {
      console.error('epub_chapter failed:', e);
    } finally {
      loading = false;
    }
  }
</script>

<article class="epub-viewer">
  <header class="book-meta">
    <h2>{title}</h2>
    {#if author}<p class="author">by {author}</p>{/if}
    <aside class="stats">{spine_len} chapters · {byte_len.toLocaleString()} bytes</aside>
  </header>
  <nav class="chapter-nav">
    <button onclick={() => loadChapter(currentChapter - 1)} disabled={currentChapter === 0} aria-label="Previous chapter">←</button>
    <span>Chapter {currentChapter + 1} / {spine_len}</span>
    <button onclick={() => loadChapter(currentChapter + 1)} disabled={currentChapter >= spine_len - 1} aria-label="Next chapter">→</button>
  </nav>
  {#if loading}
    <p class="loading">Loading chapter…</p>
  {/if}
  <div class="chapter-frame">
    <iframe srcdoc={currentXhtml} sandbox="allow-same-origin" title="{title} chapter {currentChapter + 1}" class="frame"></iframe>
  </div>
</article>

<style>
  .epub-viewer { display: flex; flex-direction: column; height: 100%; }
  .book-meta { padding: 1rem; border-bottom: 1px solid var(--border); color: var(--text-primary); }
  .book-meta h2 { margin: 0 0 0.25rem; }
  .author { color: var(--text-secondary); margin: 0 0 0.5rem; }
  .stats { color: var(--text-secondary); font-size: 0.75rem; }
  .chapter-nav { display: flex; gap: 0.5rem; align-items: center; justify-content: center; padding: 0.5rem; border-bottom: 1px solid var(--border); color: var(--text-primary); font-size: 0.85rem; }
  .chapter-nav button { cursor: pointer; padding: 0.3rem 0.6rem; background: var(--bg-secondary); color: var(--text-primary); border: 1px solid var(--border); border-radius: 0.3rem; }
  .chapter-nav button:disabled { opacity: 0.4; cursor: not-allowed; }
  .loading { text-align: center; color: var(--text-secondary); font-style: italic; padding: 1rem; }
  .chapter-frame { flex: 1; overflow: hidden; }
  .frame { width: 100%; height: 100%; border: none; background: var(--bg-primary); }
</style>