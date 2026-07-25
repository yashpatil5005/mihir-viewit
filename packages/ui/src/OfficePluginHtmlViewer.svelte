<script lang="ts">
  let { document: doc = {} }: { document?: any } = $props();

  let renderer = $derived(doc.renderer ?? {});
  let fidelity = $derived(doc.fidelity ?? null);
  let warnings = $derived(Array.isArray(doc.warnings) ? doc.warnings : []);
  let html = $derived((doc.html ?? '') as string);
</script>

<article class="office-plugin-html-viewer" data-renderer-id={renderer.id ?? ''}>
  <header class="html-meta">
    <strong>{renderer.label ?? renderer.id ?? 'Office plugin'}</strong>
    {#if renderer.version}<span>v{renderer.version}</span>{/if}
    {#if doc.byte_len}<span>{Number(doc.byte_len).toLocaleString()} bytes</span>{/if}
    <span>html: {html ? 'yes' : 'no'}</span>
    {#if fidelity?.level}<span>{fidelity.level}</span>{/if}
  </header>

  {#if warnings.length > 0}
    <details class="html-warnings">
      <summary>{warnings.length} warning(s)</summary>
      <ul>{#each warnings as warning}<li>{warning}</li>{/each}</ul>
    </details>
  {/if}

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
  .html-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.75rem;
    color: var(--text-secondary);
  }
  .html-meta strong {
    color: var(--text-primary);
  }
  .html-meta span {
    border: 1px solid var(--border);
    background: var(--bg-secondary);
    border-radius: 999px;
    padding: 0.12rem 0.45rem;
  }
  .html-warnings {
    color: var(--text-secondary);
    font-size: 0.78rem;
  }
  .html-warnings summary {
    cursor: pointer;
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
