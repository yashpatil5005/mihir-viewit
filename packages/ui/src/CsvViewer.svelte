<script lang="ts">
  // Phase 2.6 — CSV viewer. Phase 2 ships a simple virtualized table (rows
  // up to preview_rows). Phase 5 will wire a streaming fetch for later
  // pages.

  let {
    header = [],
    preview_rows = [],
    total_rows_hint,
    byte_len = 0
  }: {
    header: string[];
    preview_rows: string[][];
    total_rows_hint?: number | null;
    byte_len?: number;
  } = $props();

  let knownTotal = total_rows_hint ?? preview_rows.length;
</script>

<article class="csv-viewer">
  <aside class="meta">
    <strong>{knownTotal.toLocaleString()}{total_rows_hint == null ? '+ rows (previewing first ' + preview_rows.length + ')' : ' rows'}</strong>
    · {byte_len.toLocaleString()} bytes · csv
  </aside>
  <div class="table-frame">
    <table>
      <thead>
        <tr>
          {#each header as h, i}
            <th>{h || '#' + i}</th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each preview_rows as row, i}
          <tr>
            {#each row as cell}
              <td>{cell}</td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</article>

<style>
  .csv-viewer { padding: 0.5rem 1rem; }
  .meta { color: #888; margin-bottom: 0.5rem; font-size: 0.75rem; }
  .table-frame { overflow: auto; max-height: 75vh; border: 1px solid #e0e0e0; border-radius: 0.4rem; }
  table { border-collapse: collapse; width: 100%; font-family: ui-monospace, monospace; font-size: 0.85rem; }
  th, td { border-bottom: 1px solid #eee; padding: 0.3rem 0.6rem; text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 30ch; }
  th { background: #f5f5f5; font-weight: 600; position: sticky; top: 0; }
  tr:nth-child(even) td { background: #fafafa; }
</style>
