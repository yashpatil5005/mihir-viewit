<script lang="ts">
  // Phase 2.2 — PDF viewer. Phase 5.4 — keyboard nav (ArrowDown/Up).
  // Pages are rasterized PNG data URLs. We render them in a vertical
  // scroll with lazy loading (each page as an <img>).
  import { onMount, onDestroy } from 'svelte';
  import { pdfPage } from '@viewit/platform';

  let {
    document: docProp = {},
    source_uri = '',
  }: { document?: { page_count?: number; pages?: string[]; byte_len?: number }; source_uri?: string } = $props();

  let page_count = $derived(docProp.page_count ?? 0);
  let byte_len = $derived(docProp.byte_len ?? 0);

  let pages = $state<string[]>([...(docProp.pages ?? [])]);
  let loadingPage = $state<number | null>(null);

  $effect(() => {
    pages = [...(docProp.pages ?? [])];
  });

  async function ensurePage(i: number) {
    if (pages[i] || !source_uri || loadingPage !== null) return;
    loadingPage = i;
    try {
      const url = await pdfPage(source_uri, i);
      const next = [...pages];
      while (next.length <= i) next.push('');
      next[i] = url;
      pages = next;
    } finally {
      loadingPage = null;
    }
  }

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
  <div class="pages" bind:this={containerEl} role="document">
    {#each Array(page_count) as _, i}
      <figure class="page">
        <figcaption>Page {i + 1}</figcaption>
        {#if pages[i]}
          <img src={pages[i]} alt="Page {i + 1}" loading="lazy" />
        {:else}
          <button type="button" class="load-page" onclick={() => ensurePage(i)} disabled={loadingPage === i}>
            {loadingPage === i ? 'Rendering…' : 'Load page'}
          </button>
        {/if}
      </figure>
    {/each}
  </div>
</article>

<style>
  .pdf-viewer { padding: 0.5rem 1rem; }
  .meta { color: var(--text-secondary); margin-bottom: 1rem; font-size: 0.75rem; }
  .pages { display: flex; flex-direction: column; gap: 2rem; align-items: center; }
  .page { max-width: 100%; }
  .page figcaption { text-align: center; color: var(--text-secondary); font-size: 0.8rem; margin-bottom: 0.5rem; }
  .page img { max-width: 100%; height: auto; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }
  .load-page { padding: 0.5rem 1rem; cursor: pointer; background: var(--bg-secondary); border: 1px solid var(--border); border-radius: 0.4rem; }
</style>
