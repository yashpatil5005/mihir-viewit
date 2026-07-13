<script lang="ts">
  // Phase 3.1 — DOCX viewer. Renders blocks: paragraphs (heading-aware),
  // list items, tables, image placeholders.
  import SearchBar from './SearchBar.svelte';
  import { findAllMatches, escapeHtml } from './search';

  let { document: docProp = {} }: { document?: any } = $props();

  const blocks: Array<any> = docProp.blocks ?? [];
  const byte_len: number = docProp.byte_len ?? 0;
  let query = $state('');
  let caseSensitive = $state(false);

  function highlight(s: string): string {
    if (!query) return escapeHtml(s);
    const matches = findAllMatches(s, query, caseSensitive);
    if (matches.length === 0) return escapeHtml(s);
    let out = '';
    let cursor = 0;
    for (const m of matches) {
      out += escapeHtml(s.slice(cursor, m.index));
      out += '<mark>' + escapeHtml(s.slice(m.index, m.index + m.length)) + '</mark>';
      cursor = m.index + m.length;
    }
    out += escapeHtml(s.slice(cursor));
    return out;
  }
</script>

<article class="docx-viewer">
  <SearchBar onSearch={(q, cs) => { query = q; caseSensitive = cs; }} />
  {#if byte_len > 0}
    <aside class="meta">{byte_len.toLocaleString()} bytes</aside>
  {/if}
  <div class="body">
    {#each blocks as block}
      {#if block.kind === 'paragraph'}
        {#if block.heading}
          {@html `<h${block.heading}>` + highlight(block.text) + `</h${block.heading}>`}
        {:else}
          <p>{@html highlight(block.text)}</p>
        {/if}
      {:else if block.kind === 'list-item'}
        <ul><li style="margin-left:{block.level * 1.5}rem">{@html highlight(block.text)}</li></ul>
      {:else if block.kind === 'table'}
        <table>
          {#each block.rows as row}
            <tr>{#each row as cell}<td>{@html highlight(cell)}</td>{/each}</tr>
          {/each}
        </table>
      {:else if block.kind === 'image'}
        <div class="img-placeholder">[embedded image: {block.name}]</div>
      {/if}
    {/each}
  </div>
</article>

<style>
  .docx-viewer { padding: 1rem 2rem; max-width: 70ch; margin: 0 auto; color: var(--text-primary); }
  .meta { color: var(--text-secondary); margin-bottom: 1rem; font-size: 0.75rem; }
  .body { line-height: 1.6; }
  h1 { font-size: 1.6rem; margin: 1.2rem 0 0.6rem; }
  h2 { font-size: 1.35rem; margin: 1rem 0 0.5rem; }
  h3 { font-size: 1.15rem; margin: 0.8rem 0 0.4rem; }
  h4, h5, h6 { font-size: 1rem; margin: 0.6rem 0 0.3rem; }
  p { margin: 0.5rem 0; }
  ul { padding-left: 1.5rem; margin: 0.3rem 0; }
  li { margin: 0.2rem 0; }
  table { border-collapse: collapse; margin: 0.8rem 0; width: 100%; }
  td { border: 1px solid var(--border); padding: 0.3rem 0.5rem; font-size: 0.85rem; vertical-align: top; }
  tr:nth-child(even) td { background: var(--bg-secondary); }
  .img-placeholder { padding: 0.5rem; margin: 0.5rem 0; color: var(--text-secondary); font-style: italic; border: 1px dashed var(--border); border-radius: 0.3rem; text-align: center; font-size: 0.85rem; }
  :global(mark) { background: rgba(255, 213, 79, 0.6); border-radius: 0.15rem; }
</style>