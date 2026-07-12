<script lang="ts">
  let {
    entries = [],
    format = 'archive-zip',
    byte_len = 0
  }: {
    entries: { name: string; size: number; is_dir: boolean; compressed_size: number }[];
    format: string;
    byte_len: number;
  } = $props();
</script>

<article class="archive-viewer">
  <aside class="meta">
    <strong>{entries.length} entries</strong> · {format} · {byte_len.toLocaleString()} bytes
  </aside>
  <div class="table-frame">
    <table>
      <thead>
        <tr><th>Name</th><th>Size</th><th>Compressed</th></tr>
      </thead>
      <tbody>
        {#each entries as entry}
          <tr class:dir={entry.is_dir}>
            <td>{entry.is_dir ? '📁 ' : '📄 '}{entry.name}</td>
            <td>{entry.size.toLocaleString()}</td>
            <td>{entry.compressed_size.toLocaleString()}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</article>

<style>
  .archive-viewer { padding: 0.5rem 1rem; }
  .meta { color: #888; margin-bottom: 0.5rem; font-size: 0.75rem; }
  .table-frame { overflow: auto; max-height: 75vh; border: 1px solid #e0e0e0; border-radius: 0.4rem; }
  table { border-collapse: collapse; width: 100%; font-family: ui-monospace, monospace; font-size: 0.85rem; }
  th, td { border-bottom: 1px solid #eee; padding: 0.3rem 0.6rem; text-align: left; }
  th { background: #f5f5f5; font-weight: 600; position: sticky; top: 0; }
  .dir td { font-weight: 600; color: #444; }
</style>
