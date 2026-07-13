<script lang="ts">
  // Phase 2.2 — PDF viewer.
  // Pages are rasterized PNG data URLs. We render them in a vertical
  // scroll with lazy loading (each page as an <img>).

  let {
    page_count = 0,
    pages = [],
    byte_len = 0
  }: {
    page_count: number;
    pages: string[];
    byte_len: number;
  } = $props();
</script>

<article class="pdf-viewer">
  <aside class="meta">
    <strong>{page_count} page{page_count !== 1 ? 's' : ''}</strong> · {byte_len.toLocaleString()} bytes · pdf
  </aside>
  <div class="pages">
    {#each pages as page, i}
      <figure class="page">
        <figcaption>Page {i + 1}</figcaption>
        <img src={page} alt="Page {i + 1}" loading="lazy" />
      </figure>
    {/each}
    {#if pages.length < page_count}
      <p class="more">… and {page_count - pages.length} more pages (Phase 5 pagination)</p>
    {/if}
  </div>
</article>

<style>
  .pdf-viewer { padding: 0.5rem 1rem; }
  .meta { color: #888; margin-bottom: 1rem; font-size: 0.75rem; }
  .pages { display: flex; flex-direction: column; gap: 2rem; align-items: center; }
  .page { max-width: 100%; }
  .page figcaption { text-align: center; color: #888; font-size: 0.8rem; margin-bottom: 0.5rem; }
  .page img { max-width: 100%; height: auto; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }
  .more { text-align: center; color: #888; font-style: italic; }
</style>
