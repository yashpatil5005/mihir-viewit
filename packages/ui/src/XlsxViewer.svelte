<script lang="ts">
  // Phase 3.2 — XLSX viewer. Multi-sheet: tab strip + virtualized grid.
  import SearchBar from "./SearchBar.svelte";
  import Icon from "./Icon.svelte";
  import { findAllMatches, escapeHtml } from "./search";

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
      preview_formulas?: string[][];
      column_widths?: (number | null | undefined)[];
      cell_styles?: Array<
        Array<{
          fill_color?: string;
          font_color?: string;
          bold?: boolean;
          italic?: boolean;
          border_color?: string;
          border_top?: string;
          border_right?: string;
          border_bottom?: string;
          border_left?: string;
          number_format?: string;
          horizontal_alignment?: string;
          vertical_alignment?: string;
          wrap_text?: boolean;
        } | null>
      >;
      row_heights?: (number | null | undefined)[];
    }>,
  );
  let pluginHtml = $derived((docProp.html ?? "") as string);
  let activeSheet = $state(0);
  let query = $state("");
  let caseSensitive = $state(false);

  $effect(() => {
    if (activeSheet >= sheets.length) activeSheet = 0;
  });

  let sheet = $derived(sheets[activeSheet]);
  let columnCount = $derived.by(() => {
    if (!sheet) return 0;
    return Math.max(
      sheet.total_cols_hint ?? 0,
      sheet.header.length,
      ...sheet.preview_rows.map((row) => row.length),
    );
  });
  let columnLabels = $derived(Array.from({ length: columnCount }, (_, i) => columnLabel(i)));
  let headerCells = $derived.by(() => {
    if (!sheet) return [];
    const source = sheet.header.length > 0 ? sheet.header : columnLabels;
    return Array.from({ length: columnCount }, (_, i) => source[i] ?? "");
  });
  let formulaGrid = $derived.by(() => {
    if (!sheet) return [];
    const f = sheet.preview_formulas ?? [];
    return sheet.preview_rows.map((row, r) =>
      Array.from({ length: columnCount }, (_, i) => f[r]?.[i] ?? ""),
    );
  });
  let visibleRows = $derived.by(() => {
    if (!sheet) return [];
    return sheet.preview_rows.map((row) =>
      Array.from({ length: columnCount }, (_, i) => row[i] ?? ""),
    );
  });
  let maxRenderRows = $state(200);
  let selectedCell = $state<{ row: number; col: number; value: string; formula: string } | null>(
    null,
  );

  function handleTabKeydown(event: KeyboardEvent) {
    if (!["ArrowRight", "ArrowLeft", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    let next = activeSheet;
    if (event.key === "ArrowRight") next = (activeSheet + 1) % sheets.length;
    else if (event.key === "ArrowLeft") next = (activeSheet - 1 + sheets.length) % sheets.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = sheets.length - 1;
    activeSheet = next;
    requestAnimationFrame(() => document.getElementById(`xlsx-tab-${next}`)?.focus());
  }

  function selectCell(row: number, col: number) {
    if (selectedCell?.row === row && selectedCell.col === col) {
      selectedCell = null;
      return;
    }
    selectedCell = {
      row,
      col,
      value: visibleRows[row]?.[col] ?? "",
      formula: formulaGrid[row]?.[col] ?? "",
    };
  }

  $effect(() => {
    activeSheet;
    selectedCell = null;
  });

  function loadMoreRows() {
    maxRenderRows += 200;
  }

  let renderRows = $derived(visibleRows.slice(0, maxRenderRows));
  let hasFormulas = $derived(formulaGrid.some((row) => row.some((c) => c)));
  let formulaCount = $derived(formulaGrid.reduce((n, row) => n + row.filter((c) => c).length, 0));
  let textBody = $derived.by(() => {
    if (!sheet) return "";
    return [
      sheet.header.join("\t"),
      ...sheet.preview_rows.map((row, r) => [...row, ...(formulaGrid[r] ?? [])].join("\t")),
    ].join("\n");
  });

  let matches = $derived(query ? findAllMatches(textBody, query, caseSensitive) : []);

  function cellMatches(s: string): string {
    if (!query) return escapeHtml(s);
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

  function columnLabel(index: number): string {
    let n = index + 1;
    let label = "";
    while (n > 0) {
      const rem = (n - 1) % 26;
      label = String.fromCharCode(65 + rem) + label;
      n = Math.floor((n - 1) / 26);
    }
    return label;
  }

  // --- Enriched worksheet geometry (merged cells / column widths / frozen) ---
  function colIndexOf(letters: string): number {
    let n = 0;
    for (const ch of letters) n = n * 26 + (ch.charCodeAt(0) - 64);
    return n - 1;
  }
  function refToRect(ref: string): { r1: number; r2: number; c1: number; c2: number } {
    const [a, b] = ref.split(":");
    const end = b || a;
    const r1 = parseInt(a.match(/(\d+)$/)?.[1] ?? "1", 10);
    const r2 = parseInt(end.match(/(\d+)$/)?.[1] ?? String(r1), 10);
    const c1 = colIndexOf(a.replace(/\d+$/, ""));
    const c2 = colIndexOf(end.replace(/\d+$/, ""));
    return { r1, r2, c1, c2 };
  }

  // Map "bodyRow:col" → { colspan, rowspan } for merge anchors or { skip: true }
  // for covered cells. Body rows are preview_rows indices (excel row = idx + 2).
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let mergeMap = $derived.by((): Record<string, any> => {
    const map: Record<string, any> = {};
    if (!sheet?.merged_cells) return map;
    for (const ref of sheet.merged_cells) {
      const { r1, r2, c1, c2 } = refToRect(ref);
      const rowspan = r2 - r1 + 1;
      const colspan = c2 - c1 + 1;
      for (let r = r1; r <= r2; r++) {
        const bRow = r - 2;
        if (bRow < 0) continue;
        for (let c = c1; c <= c2; c++) {
          if (r1 === 1) {
            if (r === 2 && c === c1) map[`${bRow}:${c}`] = { colspan, rowspan: r2 - 1 };
            else if (r > 2 || c > c1) map[`${bRow}:${c}`] = { skip: true };
          } else if (r === r1 && c === c1) map[`${bRow}:${c}`] = { colspan, rowspan };
          else map[`${bRow}:${c}`] = { skip: true };
        }
      }
    }
    return map;
  });

  // Header-row merges (r1 === 1): the anchor spans header columns.
  let headerMergeMap = $derived.by(
    (): Record<number, { colspan: number; rowspan: number } | { skip: true }> => {
      const merges: Record<number, { colspan: number; rowspan: number } | { skip: true }> = {};
      if (!sheet?.merged_cells) return merges;
      for (const ref of sheet.merged_cells) {
        const { r1, r2, c1, c2 } = refToRect(ref);
        if (r1 === 1) {
          merges[c1] = { colspan: c2 - c1 + 1, rowspan: 1 };
          for (let c = c1 + 1; c <= c2; c++) merges[c] = { skip: true };
        }
      }
      return merges;
    },
  );

  function headerMerge(col: number): { colspan: number; rowspan: number } | undefined {
    const merge = headerMergeMap[col];
    return merge && !("skip" in merge) ? merge : undefined;
  }

  function headerCovered(col: number): boolean {
    const merge = headerMergeMap[col];
    return Boolean(merge && "skip" in merge);
  }

  let columnWidths = $derived((sheet?.column_widths ?? []) as (number | null | undefined)[]);
  let frozenCols = $derived(Math.max(0, sheet?.frozen_panes?.x_split ?? 0));

  function cellColSpan(bodyRow: number, col: number): number {
    const m = mergeMap[`${bodyRow}:${col}`];
    return m && !m.skip && m.colspan ? m.colspan : 1;
  }
  function cellRowSpan(bodyRow: number, col: number): number {
    const m = mergeMap[`${bodyRow}:${col}`];
    return m && !m.skip && m.rowspan ? m.rowspan : 1;
  }
  function cellCovered(bodyRow: number, col: number): boolean {
    return Boolean(mergeMap[`${bodyRow}:${col}`]?.skip);
  }

  const ROW_LABEL_PX = 3.6 * 16;
  function colStyle(col: number): string {
    const w = columnWidths[col];
    const widthPx = w ? Math.max(48, Math.round(w * 7)) : undefined;
    return widthPx ? `width:${widthPx}px;max-width:${widthPx}px;` : "";
  }
  function frozenLeft(col: number): string {
    if (col >= frozenCols) return "";
    let left = ROW_LABEL_PX;
    for (let c = 0; c < col; c++) {
      const w = columnWidths[c];
      left += w ? Math.max(48, Math.round(w * 7)) : 0;
    }
    return `left:${left}px;`;
  }

  function cellStyle(row: number, col: number): string {
    const style = sheet?.cell_styles?.[row]?.[col];
    if (!style) return "";
    return [
      style.fill_color ? `background-color:${style.fill_color}` : "",
      style.font_color ? `color:${style.font_color}` : "",
      style.bold ? "font-weight:700" : "",
      style.italic ? "font-style:italic" : "",
      style.border_color ? `border-color:${style.border_color}` : "",
      style.border_top ? `border-top:${style.border_top}` : "",
      style.border_right ? `border-right:${style.border_right}` : "",
      style.border_bottom ? `border-bottom:${style.border_bottom}` : "",
      style.border_left ? `border-left:${style.border_left}` : "",
      style.horizontal_alignment
        ? `text-align:${cssHorizontalAlignment(style.horizontal_alignment)}`
        : "",
      style.vertical_alignment
        ? `vertical-align:${cssVerticalAlignment(style.vertical_alignment)}`
        : "",
      style.wrap_text ? "white-space:normal;overflow-wrap:anywhere" : "",
    ]
      .filter(Boolean)
      .join(";");
  }

  function cssHorizontalAlignment(alignment: string): string {
    return (
      (
        {
          left: "left",
          center: "center",
          centerContinuous: "center",
          right: "right",
          fill: "left",
          justify: "justify",
          distributed: "justify",
        } as Record<string, string>
      )[alignment] ?? ""
    );
  }

  function cssVerticalAlignment(alignment: string): string {
    if (alignment === "center" || alignment === "distributed" || alignment === "justify") {
      return "middle";
    }
    return ({ top: "top", bottom: "bottom" } as Record<string, string>)[alignment] ?? "";
  }

  function numberFormat(row: number, col: number): string | undefined {
    return sheet?.cell_styles?.[row]?.[col]?.number_format;
  }

  function rowStyle(row: number): string {
    const height = sheet?.row_heights?.[row];
    return height && height > 0 ? `height:${height * (96 / 72)}px;` : "";
  }
</script>

<article class="xlsx-viewer">
  {#if pluginHtml}
    <div class="plugin-html-surface">{@html pluginHtml}</div>
  {:else}
    <SearchBar
      onSearch={(q, cs) => {
        query = q;
        caseSensitive = cs;
      }}
    />
    {#if sheets.length > 1}
      <div class="tabs" role="tablist" aria-label="Worksheets">
        {#each sheets as s, i}
          <button
            id="xlsx-tab-{i}"
            class:active={i === activeSheet}
            onclick={() => (activeSheet = i)}
            onkeydown={handleTabKeydown}
            role="tab"
            aria-selected={i === activeSheet}
            aria-controls="xlsx-tabpanel-{i}"
            tabindex={i === activeSheet ? 0 : -1}>{s.name}</button
          >
        {/each}
      </div>
    {/if}
    {#if sheet}
      <div
        id="xlsx-tabpanel-{activeSheet}"
        role={sheets.length > 1 ? "tabpanel" : undefined}
        aria-labelledby={sheets.length > 1 ? `xlsx-tab-${activeSheet}` : undefined}
      >
        <aside class="meta">
          <strong>{sheet.name}</strong>
          <span
            >{visibleRows.length}{sheet.total_rows_hint ? "/" + sheet.total_rows_hint : ""} rows</span
          >
          <span
            >{columnCount}{sheet.total_cols_hint && sheet.total_cols_hint !== columnCount
              ? "/" + sheet.total_cols_hint
              : ""} cols</span
          >
          {#if sheet.merged_cells && sheet.merged_cells.length > 0}<span
              >{sheet.merged_cells.length} merged</span
            >{/if}
          {#if sheet.frozen_panes}<span
              >frozen x:{sheet.frozen_panes.x_split} y:{sheet.frozen_panes.y_split}</span
            >{/if}
          {#if hasFormulas}<span>{formulaCount} formula{formulaCount === 1 ? "" : "s"}</span>{/if}
          {#if matches.length > 0}<span>{matches.length} matches</span>{/if}
        </aside>
        <div class="table-frame">
          <table>
            <thead>
              <tr>
                <th class="corner"></th>
                {#each columnLabels as label, i}
                  <th
                    class="col-label"
                    class:frozen-c={i < frozenCols}
                    style={colStyle(i) + frozenLeft(i)}>{label}</th
                  >
                {/each}
              </tr>
              <tr style={rowStyle(0)}>
                <th class="row-label header-row">1</th>
                {#each headerCells as h, i}
                  {#if !headerCovered(i)}
                    <th
                      colspan={headerMerge(i)?.colspan ?? 1}
                      rowspan={headerMerge(i)?.rowspan ?? 1}
                      class:frozen-c={i < frozenCols}
                      data-number-format={numberFormat(0, i)}
                      style={colStyle(i) + frozenLeft(i) + cellStyle(0, i)}
                      >{@html cellMatches(h)}</th
                    >
                  {/if}
                {/each}
              </tr>
            </thead>
            <tbody>
              {#each renderRows as row, rowIndex}
                <tr style={rowStyle(rowIndex + 1)}>
                  <th class="row-label">{rowIndex + 2}</th>
                  {#each row as cell, c}
                    {@const fmt = formulaGrid[rowIndex][c]}
                    {#if !cellCovered(rowIndex, c)}
                      <td
                        colspan={cellColSpan(rowIndex, c)}
                        rowspan={cellRowSpan(rowIndex, c)}
                        class:formula={(cell ?? "").startsWith("=") ||
                          (!(cell ?? "").trim() && !!fmt)}
                        class:frozen-c={c < frozenCols}
                        class:selected-cell={selectedCell?.row === rowIndex &&
                          selectedCell.col === c}
                        data-number-format={numberFormat(rowIndex + 1, c)}
                        style={colStyle(c) + frozenLeft(c) + cellStyle(rowIndex + 1, c)}
                        role="button"
                        tabindex="0"
                        aria-label={`Cell ${columnLabel(c)}${rowIndex + 2}: ${cell || fmt || "empty"}`}
                        onclick={() => selectCell(rowIndex, c)}
                        onkeydown={(event) => {
                          if (event.key === "Enter" || event.key === " ") {
                            event.preventDefault();
                            selectCell(rowIndex, c);
                          }
                        }}
                      >
                        {#if !(cell ?? "").trim() && fmt}
                          <span class="fx-empty">{fmt}</span>
                        {:else}
                          {@html cellMatches(cell ?? "")}
                        {/if}
                      </td>
                    {/if}
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
          {#if selectedCell}
            <div class="cell-detail" role="status" aria-live="polite" aria-atomic="true">
              <strong>{columnLabel(selectedCell.col)}{selectedCell.row + 2}</strong>
              {#if selectedCell.formula}
                <span class="cell-detail-formula">{selectedCell.formula}</span>
              {/if}
              <span class="cell-detail-value">{selectedCell.value || "(empty)"}</span>
              <button
                type="button"
                class="cell-detail-close"
                onclick={() => (selectedCell = null)}
                aria-label="Close cell detail"><Icon name="x" /></button
              >
            </div>
          {/if}
          {#if visibleRows.length > maxRenderRows}
            <div class="virtual-scroll-footer">
              <span class="virtual-scroll-status"
                >Showing {maxRenderRows} of {visibleRows.length} rows</span
              >
              <button class="load-more-btn" onclick={loadMoreRows}>Load 200 More Rows</button>
              <button class="load-more-btn" onclick={() => (maxRenderRows = visibleRows.length)}
                >Show All</button
              >
            </div>
          {/if}
        </div>
      </div>
    {/if}
  {/if}
</article>

<style>
  .xlsx-viewer {
    padding: 0.5rem 1rem;
    color: var(--text-primary);
  }
  .plugin-html-surface {
    overflow: auto;
    border-radius: 0.75rem;
  }
  .tabs {
    display: flex;
    gap: 0.3rem;
    margin-bottom: 0.5rem;
    overflow-x: auto;
  }
  .tabs button {
    cursor: pointer;
    padding: 0.36rem 0.75rem;
    background: var(--bg-secondary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    font-size: 0.8rem;
    min-height: 44px;
  }
  .tabs button.active {
    background: var(--link);
    color: #fff;
    border-color: var(--link);
    font-weight: 700;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
    color: var(--text-secondary);
    margin-bottom: 0.5rem;
    font-size: 0.75rem;
  }
  .meta strong {
    color: var(--text-primary);
    font-size: 0.86rem;
  }
  .meta span {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.12rem 0.45rem;
  }
  .table-frame {
    overflow: auto;
    max-height: 76vh;
    border: 1px solid var(--border);
    border-radius: 0.65rem;
    background: var(--bg-primary);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
  }
  table {
    border-collapse: separate;
    border-spacing: 0;
    width: max-content;
    min-width: 100%;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.82rem;
  }
  th,
  td {
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    padding: 0.34rem 0.55rem;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 34ch;
    min-width: 7ch;
  }
  thead th {
    background: var(--bg-secondary);
    font-weight: 700;
    position: sticky;
    z-index: 2;
  }
  thead tr:first-child th {
    top: 0;
    text-align: center;
    color: var(--text-secondary);
    font-size: 0.72rem;
  }
  thead tr:nth-child(2) th {
    top: 1.75rem;
  }
  .corner {
    left: 0;
    z-index: 4;
    min-width: 3.6rem;
  }
  .row-label {
    position: sticky;
    left: 0;
    z-index: 1;
    min-width: 3.6rem;
    text-align: right;
    color: var(--text-secondary);
    background: var(--bg-secondary);
    font-size: 0.72rem;
  }
  /* Frozen (sticky) leading columns from the worksheet's frozen pane. */
  .frozen-c {
    position: sticky;
    z-index: 1;
    background: var(--bg-secondary);
  }
  thead th.frozen-c {
    z-index: 3;
  }
  .col-label {
    min-width: 7ch;
  }
  tbody tr:hover td {
    background: color-mix(in srgb, var(--link) 8%, var(--bg-primary));
  }
  td.formula {
    color: var(--link);
  }
  td.selected-cell {
    outline: 2px solid var(--link);
    outline-offset: -2px;
  }
  .cell-detail {
    position: sticky;
    left: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-height: 44px;
    padding: 0.4rem 0.75rem;
    border-top: 1px solid var(--border);
    background: var(--bg-secondary);
    font-size: 0.82rem;
  }
  .cell-detail strong,
  .cell-detail-formula {
    flex-shrink: 0;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  }
  .cell-detail strong {
    color: var(--text-secondary);
  }
  .cell-detail-formula {
    color: var(--link);
  }
  .cell-detail-value {
    min-width: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .cell-detail-close {
    display: grid;
    place-items: center;
    flex: 0 0 44px;
    min-width: 44px;
    min-height: 44px;
    margin-left: auto;
    border: 0;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
  }
  .fx-empty {
    color: var(--text-secondary);
    font-style: italic;
    opacity: 0.8;
  }
  :global(mark) {
    background: rgba(255, 213, 79, 0.6);
    border-radius: 0.15rem;
  }
  .virtual-scroll-footer {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem 1rem;
    padding: 0.75rem;
    background: var(--bg-surface-secondary, rgba(255, 255, 255, 0.05));
    border-top: 1px solid var(--border-color, #30363d);
    font-size: 0.85rem;
    flex-wrap: wrap;
  }
  .virtual-scroll-status {
    flex-basis: 100%;
    text-align: center;
  }
  .load-more-btn {
    padding: 0.35rem 0.85rem;
    border-radius: 4px;
    background: var(--button-bg, #21262d);
    color: var(--text-primary, #c9d1d9);
    border: 1px solid var(--border-color, #30363d);
    cursor: pointer;
    font-weight: 500;
    min-height: 44px;
  }
  .load-more-btn:hover {
    background: var(--button-hover, #30363d);
  }
</style>
