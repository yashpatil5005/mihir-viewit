<script lang="ts">
  // Phase 3.2 — XLSX viewer. Multi-sheet: tab strip + virtualized grid.
  import SearchBar from './SearchBar.svelte';
  import { findAllMatches, escapeHtml } from './search';

  let { document: docProp = {} }: { document?: any } = $props();
  let sheets = $derived(
    (docProp.sheets ?? []) as Array<{
      name: string;
      header: string[];
      preview_rows: string[][];
      total_rows_hint?: number;
    }>,
  );
  let activeSheet = $state(0);
  let query = $state('');
  let caseSensitive = $state(false);

  $effect(() => { if (activeSheet >= sheets.length) activeSheet = 0; });

  let sheet = $derived(sheets[activeSheet]);
  let textBody = $derived.by(() => {
    if (!sheet) return '';
    return [sheet.header.join('\t'), ...sheet.preview_rows.map(r => r.join('\t'))].join('\n');
  });

  let matches = $derived(query ? findAllMatches(textBody, query, caseSensitive) : []);

  function cellMatches(s: string): string {
    if (!query) return escapeHtml(s);
    const m = findAllMatches(s, query, caseSensitive);
    if (m.length === 0) return escapeHtml(s);
    let out = ''; let c = 0;
    for (const x of m) { out += escapeHtml(s.slice(c, x.index)); out += '<mark>' + escapeHtml(s.slice(x.index, x.index + x.length)) + '</mark>'; c = x.index + x.length; }
    out += escapeHtml(s.slice(c));
    return out;
  }
</script>

<article class="xlsx-viewer">
  <SearchBar onSearch={(q, cs) => { query = q; caseSensitive = cs; }} />
  {#if sheets.length > 1}
    <nav class="tabs" role="tablist">
      {#each sheets as s, i}
        <button class:active={i === activeSheet} onclick={() => activeSheet = i} role="tab">{s.name}</button>
      {/each}
    </nav>
  {/if}
  {#if sheet}
    <aside class="meta">{sheet.preview_rows.length}{sheet.total_rows_hint ? '/' + sheet.total_rows_hint : ''} rows · {sheet.header.length} cols</aside>
    <div class="table-frame">
      <table>
        <thead>
          <tr>{#each sheet.header as h}<th>{@html cellMatches(h)}</th>{/each}</tr>
        </thead>
        <tbody>
          {#each sheet.preview_rows as row}
            <tr>{#each row as cell}<td>{@html cellMatches(cell ?? '')}</td>{/each}</tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</article>

<style>
  .xlsx-viewer { padding: 0.5rem 1rem; color: var(--text-primary); }
  .tabs { display: flex; gap: 0.3rem; margin-bottom: 0.5rem; overflow-x: auto; }
  .tabs button { cursor: pointer; padding: 0.3rem 0.7rem; background: var(--bg-secondary); color: var(--text-primary); border: 1px solid var(--border); border-radius: 0.3rem 0.3rem 0 0; font-size: 0.8rem; }
  .tabs button.active { background: var(--bg-primary); border-bottom-color: var(--bg-primary); font-weight: 600; }
  .meta { color: var(--text-secondary); margin-bottom: 0.5rem; font-size: 0.75rem; }
  .table-frame { overflow: auto; max-height: 75vh; border: 1px solid var(--border); border-radius: 0.4rem; }
  table { border-collapse: collapse; width: 100%; font-family: ui-monospace, monospace; font-size: 0.85rem; }
  th, td { border-bottom: 1px solid var(--border); padding: 0.3rem 0.6rem; text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 35ch; }
  th { background: var(--bg-secondary); font-weight: 600; position: sticky; top: 0; }
  tr:nth-child(even) td { background: var(--bg-secondary); }
  :global(mark) { background: rgba(255, 213, 79, 0.6); border-radius: 0.15rem; }
</style>