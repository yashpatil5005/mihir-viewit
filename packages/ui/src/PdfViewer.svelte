<script lang="ts">
  // Phase 2.2 — PDF viewer. Phase 5.4 — keyboard nav (ArrowDown/Up).
  // Pages are rasterized PNG data URLs. We render them in a vertical
  // scroll with lazy loading (each page as an <img>).
  import { onMount, onDestroy } from 'svelte';

  let {
    page_count = 0,
    pages = [],
    byte_len = 0
  }: {
    page_count: number;
    pages: string[];
    byte_len: number;
  } = $props();

  let containerEl: HTMLElement | null = $state(null);

  function onKey(e: KeyboardEvent) {
    if (!containerEl) return;
    const pageEls = Array.from(containerEl.querySelectorAll('.page'));
    if (pageEls.length === 0) return;
    const scrollY = window.scrollY;
    let cur = 0;
    for (let i = 0; i < pageEls.length; i++) {
      if ((pageEls[i] as HTMLElement).offsetTop > scrollY) { cur = i; break; }
    }
    let next = cur;
    if (e.key === 'ArrowDown' || e.key === 'PageDown') next = Math.min(cur + 1, pageEls.length - 1);
    else if (e.key === 'ArrowUp' || e.key === 'PageUp') next = Math.max(cur - 1, 0);
    else return;
    e.preventDefault();
    (pageEls[next] as HTMLElement).scrollIntoView({ block: 'start', behavior: 'smooth' });
  }

  onMount(() => { window.addEventListener('keydown', onKey); });
  onDestroy(() => { window.removeEventListener('keydown', onKey); });
</script>

<article class="pdf-viewer">
  <aside class="meta">
    <strong>{page_count} page{page_count !== 1 ? 's' : ''}</strong> · {byte_len.toLocaleString()} bytes · pdf
  </aside>
  <div class="pages" bind:this={containerEl} role="document" aria-label="PDF document {page_count} pages">
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
  .meta { color: var(--text-secondary); margin-bottom: 1rem; font-size: 0.75rem; }
  .pages { display: flex; flex-direction: column; gap: 2rem; align-items: center; }
  .page { max-width: 100%; }
  .page figcaption { text-align: center; color: var(--text-secondary); font-size: 0.8rem; margin-bottom: 0.5rem; }
  .page img { max-width: 100%; height: auto; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }
  .more { text-align: center; color: var(--text-secondary); font-style: italic; }
</style>
