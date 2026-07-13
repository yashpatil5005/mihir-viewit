<script lang="ts">
  // Phase 5.1 — Thumbnail / QuickLook-style grid.
  // Uses <input webkitdirectory> for desktop/web. Mobile fallback: single-file picker.
  // Image entries render <img> at 96px; non-image shows format icon + name.

  export let onPick: (file: File) => void = () => {};

  let entries: Array<{ name: string; size: number; url: string; isImage: boolean }> = [];

  function choose() {
    const input = document.createElement('input');
    input.type = 'file';
    (input as any).webkitdirectory = true;
    input.multiple = true;
    input.onchange = () => {
      const files = Array.from(input.files ?? []);
      entries = files.slice(0, 500).map((f) => {
        const type = f.type || '';
        const isImage = type.startsWith('image/');
        return {
          name: f.webkitRelativePath || f.name,
          size: f.size,
          url: isImage ? URL.createObjectURL(f) : '',
          isImage,
        };
      });
    };
    input.click();
  }

  function open(entry: typeof entries[0]) {
    // Re-read file from name match — for MVP just trigger file picker.
    const input = document.createElement('input');
    input.type = 'file';
    input.onchange = () => {
      const f = input.files?.[0];
      if (f) onPick(f);
    };
    input.click();
  }
</script>

<div class="grid-view">
  <button onclick={choose} aria-label="Browse directory">Browse…</button>
  {#if entries.length > 0}
    <div class="grid" role="grid">
      {#each entries as entry}
        <button class="cell" onclick={() => open(entry)} role="gridcell" tabindex="0">
          {#if entry.isImage}
            <img src={entry.url} alt={entry.name} loading="lazy" />
          {:else}
            <div class="icon">{ext(entry.name)}</div>
          {/if}
          <span class="label">{basename(entry.name)}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<script context="module" lang="ts">
  function basename(p: string) {
    const i = p.lastIndexOf('/');
    return i >= 0 ? p.slice(i + 1) : p;
  }
  function ext(p: string) {
    const i = p.lastIndexOf('.');
    return i >= 0 ? p.slice(i + 1).toUpperCase() : '?';
  }
</script>

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