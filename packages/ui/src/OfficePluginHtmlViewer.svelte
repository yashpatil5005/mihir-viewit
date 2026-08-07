<script lang="ts">
  let { document: doc = {} }: { document?: any } = $props();

  let html = $derived((doc.html ?? "") as string);
</script>

<article class="office-plugin-html-viewer">
  {#if html}
    <div class="office-plugin-html-host">{@html html}</div>
  {:else}
    <p class="html-empty">Plugin returned no HTML render surface.</p>
  {/if}
</article>

<style>
  .office-plugin-html-viewer {
    display: grid;
    gap: 0.6rem;
    color: var(--text-primary);
  }
  .office-plugin-html-host {
    overflow: auto;
    border-radius: 0.8rem;
  }
  .html-empty {
    color: var(--text-secondary);
    border: 1px dashed var(--border);
    border-radius: 0.7rem;
    padding: 1rem;
  }
</style>
