<script lang="ts">
  // Phase 4.6 — .desktop entry viewer. Renders Key=Value pairs with field-name
  // highlighting and section headers ([Desktop Entry] / [Desktop Action ...]).
  let { content = '' }: { content?: string } = $props();
  let lines = $derived(content.split(/\r?\n/));
</script>

<article class="desktop-viewer">
  <pre class="content">
    {#each lines as line}
      {#if /^\[.*\]$/.test(line)}
        <span class="section">{line}</span>
      {:else if /^[\w-]+=/.test(line)}
        <span class="key">{line.replace(/=.*$/, '')}</span><span class="eq">=</span><span class="val">{line.replace(/^[\w-]+=/, '')}</span>
      {:else}
        <span>{line}</span>
      {/if}
      {'\n'}
    {/each}
  </pre>
</article>

<style>
  .desktop-viewer { padding: 0.5rem 1rem; }
  .content { padding: 0.75rem 1rem; background: var(--bg-secondary); color: var(--text-primary); border-radius: 0.4rem; overflow: auto; white-space: pre-wrap; word-break: break-word; font-family: ui-monospace, monospace; font-size: 0.875rem; }
  .section { color: var(--link); font-weight: 600; }
  .key { color: var(--text-secondary); font-weight: 600; }
  .eq { color: var(--text-secondary); }
  .val { color: var(--text-primary); }
</style>