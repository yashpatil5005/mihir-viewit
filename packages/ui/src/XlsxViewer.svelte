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
      total_cols_hint?: number;
      merged_cells?: string[];
      frozen_panes?: { x_split: number; y_split: number; active_pane: string };
    }>,
  );
  let pluginHtml = $derived((docProp.html ?? '') as string);
  let activeSheet = $state(0);
  let query = $state('');
  let caseSensitive = $state(false);

  $effect(() => { if (activeSheet >= sheets.length) activeSheet = 0; });

  let sheet = $derived(sheets[activeSheet]);
  let columnCount = $derived.by(() => {
    if (!sheet) return 0;
    return Math.max(sheet.total_cols_hint ?? 0, sheet.header.length, ...sheet.preview_rows.map((row) => row.length));
  });
  let columnLabels = $derived(Array.from({ length: columnCount }, (_, i) => columnLabel(i)));
  let headerCells = $derived.by(() => {
    if (!sheet) return [];
    const source = sheet.header.length > 0 ? sheet.header : columnLabels;
    return Array.from({ length: columnCount }, (_, i) => source[i] ?? '');
  });
  let visibleRows = $derived.by(() => {
    if (!sheet) return [];
    return sheet.preview_rows.map((row) => Array.from({ length: columnCount }, (_, i) => row[i] ?? ''));
  });
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

  function columnLabel(index: number): string {
    let n = index + 1;
    let label = '';
    while (n > 0) {
      const rem = (n - 1) % 26;
      label = String.fromCharCode(65 + rem) + label;
      n = Math.floor((n - 1) / 26);
    }
    return label;
  }
</script>

<article class="xlsx-viewer">
  {#if pluginHtml}
    <div class="plugin-html-surface">{@html pluginHtml}</div>
  {:else}
  <SearchBar onSearch={(q, cs) => { query = q; caseSensitive = cs; }} />
  {#if sheets.length > 1}
    <div class="tabs" role="tablist">
      {#each sheets as s, i}
        <button class:active={i === activeSheet} onclick={() => activeSheet = i} role="tab">{s.name}</button>
      {/each}
    </div>
  {/if}
  {#if sheet}
    <aside class="meta">
      <strong>{sheet.name}</strong>
      <span>{visibleRows.length}{sheet.total_rows_hint ? '/' + sheet.total_rows_hint : ''} rows</span>
      <span>{columnCount}{sheet.total_cols_hint && sheet.total_cols_hint !== columnCount ? '/' + sheet.total_cols_hint : ''} cols</span>
      {#if sheet.merged_cells && sheet.merged_cells.length > 0}<span>{sheet.merged_cells.length} merged</span>{/if}
      {#if sheet.frozen_panes}<span>frozen x:{sheet.frozen_panes.x_split} y:{sheet.frozen_panes.y_split}</span>{/if}
      {#if matches.length > 0}<span>{matches.length} matches</span>{/if}
    </aside>
    <div class="table-frame">
      <table>
        <thead>
          <tr>
            <th class="corner"></th>
            {#each columnLabels as label}
              <th class="col-label">{label}</th>
            {/each}
          </tr>
          <tr>
            <th class="row-label header-row">1</th>
            {#each headerCells as h}<th>{@html cellMatches(h)}</th>{/each}
          </tr>
        </thead>
        <tbody>
          {#each visibleRows as row, rowIndex}
            <tr>
              <th class="row-label">{rowIndex + 2}</th>
              {#each row as cell}
                <td class:formula={(cell ?? '').startsWith('=')} title={cell}>{@html cellMatches(cell ?? '')}</td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
  {/if}
</article>

<style>
  .xlsx-viewer { padding: 0.5rem 1rem; color: var(--text-primary); }
  .plugin-html-surface { overflow: auto; border-radius: 0.75rem; }
  .tabs { display: flex; gap: 0.3rem; margin-bottom: 0.5rem; overflow-x: auto; }
  .tabs button { cursor: pointer; padding: 0.36rem 0.75rem; background: var(--bg-secondary); color: var(--text-primary); border: 1px solid var(--border); border-radius: 0.5rem; font-size: 0.8rem; }
  .tabs button.active { background: var(--link); color: #fff; border-color: var(--link); font-weight: 700; }
  .meta { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: center; color: var(--text-secondary); margin-bottom: 0.5rem; font-size: 0.75rem; }
  .meta strong { color: var(--text-primary); font-size: 0.86rem; }
  .meta span { background: var(--bg-secondary); border: 1px solid var(--border); border-radius: 999px; padding: 0.12rem 0.45rem; }
  .table-frame { overflow: auto; max-height: 76vh; border: 1px solid var(--border); border-radius: 0.65rem; background: var(--bg-primary); box-shadow: inset 0 1px 0 rgba(255,255,255,0.04); }
  table { border-collapse: separate; border-spacing: 0; width: max-content; min-width: 100%; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: 0.82rem; }
  th, td { border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); padding: 0.34rem 0.55rem; text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 34ch; min-width: 7ch; }
  thead th { background: var(--bg-secondary); font-weight: 700; position: sticky; z-index: 2; }
  thead tr:first-child th { top: 0; text-align: center; color: var(--text-secondary); font-size: 0.72rem; }
  thead tr:nth-child(2) th { top: 1.75rem; }
  .corner { left: 0; z-index: 4; min-width: 3.6rem; }
  .row-label { position: sticky; left: 0; z-index: 1; min-width: 3.6rem; text-align: right; color: var(--text-secondary); background: var(--bg-secondary); font-size: 0.72rem; }
  .col-label { min-width: 7ch; }
  tbody tr:hover td { background: color-mix(in srgb, var(--link) 8%, var(--bg-primary)); }
  td.formula { color: var(--link); }
  :global(mark) { background: rgba(255, 213, 79, 0.6); border-radius: 0.15rem; }
</style>
