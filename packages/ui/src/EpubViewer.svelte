<script lang="ts">
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
</script>

<article class="epub-viewer">
  <header class="book-meta">
    <h2>{title}</h2>
    {#if author}<p class="author">by {author}</p>{/if}
    <aside class="stats">{spine_len} chapters · {byte_len.toLocaleString()} bytes</aside>
  </header>
  <div class="chapter-frame">
    {@html first_chapter_xhtml}
  </div>
</article>

<style>
  .epub-viewer { display: flex; flex-direction: column; height: 100%; }
  .book-meta { padding: 1rem; border-bottom: 1px solid var(--border); }
  .book-meta h2 { margin: 0 0 0.25rem; }
  .author { color: var(--text-secondary); margin: 0 0 0.5rem; }
  .stats { color: var(--text-secondary); font-size: 0.75rem; }
  .chapter-frame { flex: 1; padding: 2rem; overflow: auto; max-width: 45ch; margin: 0 auto; line-height: 1.6; }
</style>
