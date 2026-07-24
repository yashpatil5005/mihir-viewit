<script lang="ts">
  // Phase 3.1 — DOCX viewer. Renders blocks: paragraphs (heading-aware),
  // list items, tables, image placeholders.
  import SearchBar from './SearchBar.svelte';
  import { findAllMatches, escapeHtml } from './search';

  let { document: docProp = {} }: { document?: any } = $props();

  let blocks = $derived((docProp.blocks ?? []) as Array<any>);
  let byte_len = $derived((docProp.byte_len ?? 0) as number);
  let query = $state('');
  let caseSensitive = $state(false);

  let outline = $derived(blocks
    .map((block, index) => ({ ...block, index }))
    .filter((block) => block.kind === 'paragraph' && block.heading && block.text));
  let stats = $derived.by(() => {
    const text = blocks.map((block) => {
      if (block.kind === 'table') return (block.rows ?? []).flat().join(' ');
      return block.text ?? '';
    }).join(' ');
    const words = text.trim() ? text.trim().split(/\s+/).length : 0;
    const tables = blocks.filter((block) => block.kind === 'table').length;
    return { words, tables, blocks: blocks.length };
  });

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

  function headingTag(level: number): 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6' {
    const safeLevel = Math.max(1, Math.min(6, Number(level) || 1));
    return `h${safeLevel}` as 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6';
  }
</script>

<article class="docx-viewer">
  <div class="toolbar">
    <SearchBar onSearch={(q, cs) => { query = q; caseSensitive = cs; }} />
    <aside class="meta">
      <span>{stats.words.toLocaleString()} words</span>
      <span>{stats.blocks.toLocaleString()} blocks</span>
      {#if stats.tables > 0}<span>{stats.tables} tables</span>{/if}
      {#if byte_len > 0}<span>{byte_len.toLocaleString()} bytes</span>{/if}
    </aside>
  </div>

  <div class="layout">
    {#if outline.length > 0}
      <nav class="outline" aria-label="Document outline">
        <strong>Outline</strong>
        {#each outline as item}
          <a href={`#block-${item.index}`} style={`padding-left:${Math.max(0, (item.heading ?? 1) - 1) * 0.75}rem`}>{item.text}</a>
        {/each}
      </nav>
    {/if}

    <div class="page">
      <div class="body">
        {#each blocks as block, index}
          {#if block.kind === 'paragraph'}
            {#if block.heading}
              <svelte:element this={headingTag(block.heading)} id={`block-${index}`}>{@html highlight(block.text)}</svelte:element>
            {:else}
              <p id={`block-${index}`}>{@html highlight(block.text)}</p>
            {/if}
          {:else if block.kind === 'list-item'}
            <ul id={`block-${index}`} style={`--level:${block.level ?? 0}`}><li>{@html highlight(block.text)}</li></ul>
          {:else if block.kind === 'table'}
            <div class="table-wrap" id={`block-${index}`}>
              <table>
                <tbody>
                  {#each block.rows as row}
                    <tr>{#each row as cell}<td>{@html highlight(cell)}</td>{/each}</tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {:else if block.kind === 'image'}
            <figure class="img-placeholder" id={`block-${index}`}>Embedded image: {block.name}</figure>
          {/if}
        {/each}
      </div>
    </div>
  </div>
</article>

<style>
  .docx-viewer { color: var(--text-primary); }
  .toolbar { position: sticky; top: 3.2rem; z-index: 3; background: var(--bg-primary); border-bottom: 1px solid var(--border); padding: 0.75rem 0 0.6rem; }
  .meta { display: flex; flex-wrap: wrap; gap: 0.4rem; color: var(--text-secondary); margin-top: 0.5rem; font-size: 0.75rem; }
  .meta span { background: var(--bg-secondary); border: 1px solid var(--border); border-radius: 999px; padding: 0.12rem 0.45rem; }
  .layout { display: grid; grid-template-columns: minmax(10rem, 16rem) minmax(0, 1fr); gap: 1rem; align-items: start; margin-top: 1rem; }
  .outline { position: sticky; top: 8rem; max-height: calc(100vh - 9rem); overflow: auto; border: 1px solid var(--border); border-radius: 0.75rem; padding: 0.75rem; background: var(--bg-secondary); }
  .outline strong { display: block; font-size: 0.78rem; margin-bottom: 0.5rem; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.08em; }
  .outline a { display: block; color: var(--text-primary); text-decoration: none; font-size: 0.8rem; line-height: 1.25; padding: 0.22rem 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .outline a:hover { color: var(--link); }
  .page { max-width: 76ch; margin: 0 auto 2rem; background: var(--bg-primary); border: 1px solid var(--border); border-radius: 0.9rem; box-shadow: 0 0.8rem 2rem rgba(0, 0, 0, 0.08); }
  .body { line-height: 1.68; padding: clamp(1rem, 4vw, 2.5rem); }
  .body :global(h1) { font-size: 1.8rem; margin: 1.4rem 0 0.7rem; letter-spacing: -0.02em; }
  .body :global(h2) { font-size: 1.45rem; margin: 1.2rem 0 0.6rem; letter-spacing: -0.01em; }
  .body :global(h3) { font-size: 1.2rem; margin: 1rem 0 0.45rem; }
  .body :global(h4), .body :global(h5), .body :global(h6) { font-size: 1rem; margin: 0.75rem 0 0.35rem; }
  p { margin: 0.58rem 0; }
  ul { padding-left: calc(1.35rem + var(--level, 0) * 1rem); margin: 0.28rem 0; }
  li { margin: 0.22rem 0; }
  .table-wrap { overflow: auto; margin: 1rem 0; border: 1px solid var(--border); border-radius: 0.55rem; }
  table { border-collapse: collapse; width: 100%; }
  td { border: 1px solid var(--border); padding: 0.42rem 0.6rem; font-size: 0.86rem; vertical-align: top; }
  tr:nth-child(even) td { background: var(--bg-secondary); }
  .img-placeholder { padding: 0.75rem; margin: 0.75rem 0; color: var(--text-secondary); font-style: italic; border: 1px dashed var(--border); border-radius: 0.5rem; text-align: center; font-size: 0.85rem; background: var(--bg-secondary); }
  :global(mark) { background: rgba(255, 213, 79, 0.6); border-radius: 0.15rem; }
  @media (max-width: 800px) { .layout { display: block; } .outline { display: none; } .toolbar { top: 3rem; } .page { border-radius: 0.65rem; } }
</style>
