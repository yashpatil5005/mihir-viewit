<script lang="ts">
  import { pickSingleFile } from '@viewit/platform';

  let { onPick, root = 'desktop' }: { onPick: (file: File) => void; root?: string } = $props();

  let entries: Array<{ name: string; size: number; url: string; isImage: boolean }> = $state([]);

  function basename(p: string) {
    const i = p.lastIndexOf('/');
    return i >= 0 ? p.slice(i + 1) : p;
  }
  function extLabel(p: string) {
    const i = p.lastIndexOf('.');
    return i >= 0 ? p.slice(i + 1).toUpperCase() : '?';
  }

  async function choose() {
    if (root === 'mobile' || root === 'desktop') {
      const f = await pickSingleFile();
      if (f) onPick(f);
      return;
    }
    const input = document.createElement('input');
    input.type = 'file';
    (input as HTMLInputElement & { webkitdirectory?: boolean }).webkitdirectory = true;
    input.multiple = true;
    input.onchange = () => {
      const files = Array.from(input.files ?? []);
      entries = files.slice(0, 500).map((f) => {
        const type = f.type || '';
        const isImage = type.startsWith('image/');
        return {
          name: (f as File & { webkitRelativePath?: string }).webkitRelativePath || f.name,
          size: f.size,
          url: isImage ? URL.createObjectURL(f) : '',
          isImage,
        };
      });
    };
    input.click();
  }

  async function openEntry() {
    const f = await pickSingleFile();
    if (f) onPick(f);
  }
</script>

<div class="grid-view">
  <button type="button" onclick={choose} aria-label="Choose file">Choose file…</button>
  {#if entries.length > 0}
    <div class="grid" role="grid">
      {#each entries as entry}
        <button type="button" class="cell" onclick={openEntry} role="gridcell" tabindex="0">
          {#if entry.isImage}
            <img src={entry.url} alt={entry.name} loading="lazy" />
          {:else}
            <div class="icon">{extLabel(entry.name)}</div>
          {/if}
          <span class="label">{basename(entry.name)}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .grid-view { padding: 1rem; }
  .grid-view > button { cursor: pointer; margin-bottom: 1rem; background: var(--bg-secondary); color: var(--text-primary); border: 1px solid var(--border); border-radius: 0.3rem; padding: 0.4rem 0.8rem; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 1rem; }
  .cell { display: flex; flex-direction: column; gap: 0.4rem; padding: 0.5rem; background: var(--bg-primary); border: 1px solid var(--border); border-radius: 0.4rem; cursor: pointer; text-align: center; }
  .cell:hover { background: var(--bg-secondary); }
  .cell img { width: 100%; height: 96px; object-fit: cover; border-radius: 0.3rem; }
  .icon { height: 96px; display: flex; align-items: center; justify-content: center; background: var(--bg-secondary); border-radius: 0.3rem; font-family: ui-monospace, monospace; font-weight: 600; color: var(--text-secondary); }
  .label { font-size: 0.75rem; color: var(--text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>