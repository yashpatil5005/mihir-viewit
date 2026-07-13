<script lang="ts">
  // Phase 2.9 — Archive viewer with drill-down.
  // Tap on a file entry → extract to a temp object URL → re-route through
  // openFile() so the inner format's viewer renders.

  let {
    entries = [],
    format = 'archive-zip',
    byte_len = 0
  }: {
    entries: { name: string; size: number; is_dir: boolean; compressed_size: number }[];
    format: string;
    byte_len: number;
  } = $props();

  import { openFile } from '@viewit/platform';

  async function drill(entry: any) {
    if (entry.is_dir) return;
    // Phase 2.9 — best-effort drill-down: re-fetches via Tauri command
    // (archive_extract) or web fallback. Web path: fetch+zip lib heavy; skip.
    if (typeof (window as any).__TAURI_INTERNALS__ === 'undefined') {
      alert(`Drill-down requires Tauri. Entry: ${entry.name}`);
      return;
    }
    try {
      const invoke = (await import('@tauri-apps/api/core')).invoke;
      const bytes: number[] = await invoke('archive_extract', {
        uri: (window as any).__viewit_uri__,
        entryName: entry.name,
      });
      const ext = entry.name.split('.').pop()?.toLowerCase() ?? '';
      const blob = new Blob([new Uint8Array(bytes)], { type: 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      await openFile(url);
      URL.revokeObjectURL(url);
    } catch (e) {
      console.error('archive drill-down failed:', e);
      alert(`Failed to extract ${entry.name}: ${e}`);
    }
  }
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
            <td>
              {#if entry.is_dir}
                <span>📁 {entry.name}</span>
              {:else}
                <button class="drill" onclick={() => drill(entry)} aria-label="Open {entry.name}">
                  📄 {entry.name}
                </button>
              {/if}
            </td>
            <td>{entry.size.toLocaleString()}</td>
            <td>{entry.compressed_size.toLocaleString()}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</article>

<style>
  .archive-viewer { padding: 0.5rem 1rem; color: var(--text-primary); }
  .meta { color: var(--text-secondary); margin-bottom: 0.5rem; font-size: 0.75rem; }
  .table-frame { overflow: auto; max-height: 75vh; border: 1px solid var(--border); border-radius: 0.4rem; }
  table { border-collapse: collapse; width: 100%; font-family: ui-monospace, monospace; font-size: 0.85rem; }
  th, td { border-bottom: 1px solid var(--border); padding: 0.3rem 0.6rem; text-align: left; }
  th { background: var(--bg-secondary); font-weight: 600; position: sticky; top: 0; }
  .dir td { font-weight: 600; color: var(--text-primary); }
  .drill { cursor: pointer; background: none; border: none; padding: 0; color: var(--link); font-family: inherit; font-size: inherit; text-align: left; }
  .drill:hover { text-decoration: underline; }
</style>