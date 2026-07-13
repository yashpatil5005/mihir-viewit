<script lang="ts">
  export let document: any;

  const slides: Array<{ title: string; body: string }> = document.slides ?? [];
</script>

<div class="pptx-viewer">
  <div class="pptx-meta">
    {document.slide_count} slide{(document.slide_count ?? 0) === 1 ? '' : 's'}
  </div>
  {#each slides as slide, i}
    <div class="pptx-slide">
      <div class="slide-number">Slide {i + 1}</div>
      {#if slide.title}
        <h2 class="slide-title">{slide.title}</h2>
      {/if}
      {#if slide.body}
        <div class="slide-body">{slide.body}</div>
      {/if}
      {#if !slide.title && !slide.body}
        <div class="slide-empty">No text content</div>
      {/if}
    </div>
  {/each}
</div>

<style>
  .pptx-viewer {
    padding: 1rem;
    max-width: 800px;
    margin: 0 auto;
  }
  .pptx-meta {
    font-size: 0.875rem;
    color: var(--text-secondary);
    margin-bottom: 1rem;
  }
  .pptx-slide {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 1rem;
    margin-bottom: 1rem;
    background: var(--bg-secondary);
  }
  .slide-number {
    font-size: 0.75rem;
    color: var(--text-secondary);
    margin-bottom: 0.5rem;
  }
  .slide-title {
    font-size: 1.25rem;
    margin: 0 0 0.5rem 0;
    color: var(--text-primary);
  }
  .slide-body {
    font-size: 0.9375rem;
    color: var(--text-primary);
    white-space: pre-wrap;
    line-height: 1.5;
  }
  .slide-empty {
    color: var(--text-secondary);
    font-style: italic;
    font-size: 0.875rem;
  }
</style>
