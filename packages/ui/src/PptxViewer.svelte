<script lang="ts">
  // Phase 3.3 — PPTX viewer with slide carousel.
  import { onMount, onDestroy } from 'svelte';

  let { document: docProp = {} }: { document?: any } = $props();
  const slides: Array<{ title: string; body: string }> = docProp.slides ?? [];
  const slide_count: number = docProp.slide_count ?? 0;

  let active = $state(0);

  function prev() { active = Math.max(0, active - 1); }
  function next() { active = Math.min(slides.length - 1, active + 1); }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') { e.preventDefault(); next(); }
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') { e.preventDefault(); prev(); }
  }
  onMount(() => window.addEventListener('keydown', onKey));
  onDestroy(() => window.removeEventListener('keydown', onKey));

  let slide = $derived(slides[active]);

  let visibleThumbs: number[] = $derived.by(() => {
    if (slides.length <= 20) return slides.map((_, i) => i);
    const start = Math.max(0, active - 3);
    const end = Math.min(slides.length, start + 6);
    return Array.from({length: end - start}, (_, k) => start + k);
  });
</script>

<div class="pptx-wrapper" role="region" aria-label="Slide carousel {slide_count} slides">
  <div class="meta">{slide_count} slide{(slide_count ?? 0) === 1 ? '' : 's'} · slide {active + 1} of {slides.length}</div>
  {#if slide}
    <div class="slide" role="group" aria-label="Slide {active + 1}">
      {#if slide.title}
        <h2>{slide.title}</h2>
      {/if}
      {#if slide.body}
        <div class="body">{slide.body}</div>
      {/if}
      {#if !slide.title && !slide.body}
        <div class="empty">(No extractable text on this slide)</div>
      {/if}
    </div>
  {:else}
    <p class="empty">No slides extracted.</p>
  {/if}
  <div class="controls">
    <button onclick={prev} disabled={active === 0} aria-label="Previous slide">←</button>
    <button onclick={next} disabled={active >= slides.length - 1} aria-label="Next slide">→</button>
  </div>
  {#if slides.length > 1}
    <div class="thumbs">
      {#each visibleThumbs as i}
        <button class:active={i === active} onclick={() => active = i} aria-label="Go to slide {i + 1}">
          <span class="num">{i + 1}</span>
          <span class="thumb-title">{slides[i].title || '(no title)'}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .pptx-wrapper { padding: 1rem; max-width: 900px; margin: 0 auto; color: var(--text-primary); }
  .meta { font-size: 0.8rem; color: var(--text-secondary); margin-bottom: 1rem; }
  .slide { border: 1px solid var(--border); border-radius: 8px; padding: 2rem 2.5rem; background: var(--bg-secondary); min-height: 300px; }
  .slide h2 { font-size: 1.5rem; margin: 0 0 1rem 0; border-bottom: 2px solid var(--link); padding-bottom: 0.5rem; }
  .slide .body { font-size: 1rem; line-height: 1.6; white-space: pre-wrap; }
  .controls { display: flex; gap: 0.5rem; justify-content: center; margin: 1rem 0; }
  .controls button { cursor: pointer; padding: 0.4rem 1rem; background: var(--bg-secondary); color: var(--text-primary); border: 1px solid var(--border); border-radius: 0.3rem; font-size: 1rem; }
  .controls button:disabled { opacity: 0.4; cursor: not-allowed; }
  .thumbs { display: flex; gap: 0.5rem; overflow-x: auto; padding: 0.5rem 0; }
  .thumbs button { cursor: pointer; padding: 0.4rem 0.6rem; background: var(--bg-primary); color: var(--text-primary); border: 1px solid var(--border); border-radius: 0.3rem; min-width: 80px; max-width: 140px; text-align: left; }
  .thumbs button.active { border-color: var(--link); background: var(--bg-secondary); }
  .num { font-size: 0.65rem; color: var(--text-secondary); display: block; margin-bottom: 0.2rem; }
  .thumb-title { font-size: 0.8rem; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .empty { color: var(--text-secondary); font-style: italic; }
</style>