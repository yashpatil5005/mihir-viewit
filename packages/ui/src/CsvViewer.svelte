<script lang="ts">
  // CSV viewer — fixed-row-height virtualized table with search highlighting.
  // Only the visible window of rows is rendered; scrolling updates the window
  // via rAF-throttled scrollTop reads so huge files stay smooth on mobile.

  import { findAllMatches, escapeHtml } from "./search";

  let {
    header = [],
    preview_rows = [],
    total_rows_hint,
    byte_len = 0,
    searchQuery = "",
    searchCaseSensitive = false,
    searchNav = 0,
    onSearchResult = (_count: number, _current: number) => {},
  }: {
    header: string[];
    preview_rows: string[][];
    total_rows_hint?: number | null;
    byte_len?: number;
    searchQuery?: string;
    searchCaseSensitive?: boolean;
    searchNav?: number;
    onSearchResult?: (count: number, current: number) => void;
  } = $props();

  let knownTotal = $derived(total_rows_hint ?? preview_rows.length);

  // --- Search ---------------------------------------------------------------
  let query = $state("");
  let caseSensitive = $state(false);
  let matches: Array<{ row: number; col: number }> = $state([]);
  let currentIdx = $state(0);
  let lastNav = 0;

  $effect(() => {
    const q = searchQuery;
    const cs = searchCaseSensitive;
    if (q === query && cs === caseSensitive) return;
    query = q;
    caseSensitive = cs;
    const found: Array<{ row: number; col: number }> = [];
    if (q) {
      for (let r = 0; r < preview_rows.length; r++) {
        const row = preview_rows[r];
        for (let c = 0; c < row.length; c++) {
          const cell = row[c] ?? "";
          const hay = cs ? cell : cell.toLowerCase();
          const needle = cs ? q : q.toLowerCase();
          if (hay.includes(needle)) found.push({ row: r, col: c });
        }
      }
    }
    matches = found;
    currentIdx = 0;
    onSearchResult(matches.length, matches.length > 0 ? 1 : 0);
    if (matches.length > 0) scrollToMatch(0);
  });

  $effect(() => {
    const nav = searchNav;
    if (nav === lastNav || matches.length === 0) return;
    const direction = nav > lastNav ? 1 : -1;
    lastNav = nav;
    currentIdx = (((currentIdx + direction) % matches.length) + matches.length) % matches.length;
    onSearchResult(matches.length, currentIdx + 1);
    scrollToMatch(currentIdx);
  });

  function scrollToMatch(idx: number) {
    const m = matches[idx];
    if (!m || !frameEl) return;
    const target = m.row * ROW_H;
    frameEl.scrollTo({ top: Math.max(0, target - frameEl.clientHeight / 2), behavior: "smooth" });
  }

  function cellHtml(row: number, col: number): string {
    const s = preview_rows[row]?.[col] ?? "";
    if (!query) return escapeHtml(s);
    return highlightCell(s);
  }

  function highlightCell(s: string): string {
    const m = findAllMatches(s, query, caseSensitive);
    if (m.length === 0) return escapeHtml(s);
    let out = "";
    let c = 0;
    for (const x of m) {
      out += escapeHtml(s.slice(c, x.index));
      out += "<mark>" + escapeHtml(s.slice(x.index, x.index + x.length)) + "</mark>";
      c = x.index + x.length;
    }
    out += escapeHtml(s.slice(c));
    return out;
  }

  // --- Virtualization -------------------------------------------------------
  const ROW_H = 34;
  const OVERSCAN = 8;
  let frameEl: HTMLDivElement | null = $state(null);
  let scrollTop = $state(0);
  let frameH = $state(600);
  let rafPending = false;

  let startRow = $derived(Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN));
  let endRow = $derived(
    Math.min(preview_rows.length, Math.ceil((scrollTop + frameH) / ROW_H) + OVERSCAN),
  );
  let topPad = $derived(startRow * ROW_H);
  let bottomPad = $derived(Math.max(0, (preview_rows.length - endRow) * ROW_H));

  function onScroll() {
    if (rafPending || !frameEl) return;
    rafPending = true;
    requestAnimationFrame(() => {
      rafPending = false;
      if (!frameEl) return;
      scrollTop = frameEl.scrollTop;
      frameH = frameEl.clientHeight;
    });
  }

  $effect(() => {
    if (frameEl && frameH !== frameEl.clientHeight) frameH = frameEl.clientHeight;
  });
</script>

<article class="csv-viewer">
  <aside class="meta">
    <strong
      >{knownTotal.toLocaleString()}{total_rows_hint == null
        ? "+ rows (previewing first " + preview_rows.length + ")"
        : " rows"}</strong
    >
    · {byte_len.toLocaleString()} bytes · csv
    {#if matches.length > 0}<span class="match-pill">{matches.length} matches</span>{/if}
  </aside>
  <div class="table-frame" bind:this={frameEl} onscroll={onScroll}>
    <table>
      <thead>
        <tr>
          <th class="row-num">#</th>
          {#each header as h, i}
            <th>{h || "#" + i}</th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#if topPad > 0}
          <tr class="spacer" style="height:{topPad}px"><td colspan={header.length + 1}></td></tr>
        {/if}
        {#each preview_rows.slice(startRow, endRow) as row, i}
          {@const rowIndex = startRow + i}
          <tr class:selected={matches[currentIdx]?.row === rowIndex}>
            <td class="row-num">{rowIndex + 1}</td>
            {#each row as _cell, c}
              <td>{@html cellHtml(rowIndex, c)}</td>
            {/each}
          </tr>
        {/each}
        {#if bottomPad > 0}
          <tr class="spacer" style="height:{bottomPad}px"><td colspan={header.length + 1}></td></tr>
        {/if}
      </tbody>
    </table>
  </div>
</article>

<style>
  .csv-viewer {
    padding: 0.5rem 1rem;
  }
  .meta {
    color: var(--text-secondary);
    margin-bottom: 0.5rem;
    font-size: 0.75rem;
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  .match-pill {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.12rem 0.45rem;
  }
  .table-frame {
    overflow: auto;
    max-height: 75vh;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    contain: strict;
  }
  table {
    border-collapse: collapse;
    width: max-content;
    min-width: 100%;
    font-family: ui-monospace, monospace;
    font-size: 0.85rem;
  }
  th,
  td {
    border-bottom: 1px solid var(--border);
    padding: 0.3rem 0.6rem;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 30ch;
    height: 34px; /* keep in sync with ROW_H */
  }
  tr.spacer td {
    border-bottom: 0;
    padding: 0;
  }
  th {
    background: var(--bg-secondary);
    font-weight: 600;
    position: sticky;
    top: 0;
    z-index: 2;
  }
  .row-num {
    position: sticky;
    left: 0;
    z-index: 1;
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.72rem;
    text-align: right;
    min-width: 3.2rem;
  }
  thead .row-num {
    z-index: 3;
  }
  tr:nth-child(even) td:not(.row-num) {
    background: color-mix(in srgb, var(--bg-secondary) 55%, transparent);
  }
  tr.selected td {
    outline: 2px solid var(--link);
    outline-offset: -2px;
  }
  :global(mark) {
    background: rgba(255, 213, 79, 0.6);
    border-radius: 0.15rem;
  }
</style>
