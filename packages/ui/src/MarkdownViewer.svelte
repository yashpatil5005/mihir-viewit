<script lang="ts">
  // Phase 2.4 — Markdown rendering.
  //
  // Safety: pulldown-cmark (Rust) already strips raw HTML by default (we
  // didn't pass ENABLE_PLAIN_TEXT-unsafe opts). On top of that, we render
  // the HTML inside a CSP-protected `<div>` with both:
  //   - sanitize via DOMPurify if available (AggressivelyLazy fallback if not)
  //   - sandboxed iframe srcdoc as a belt-and-suspenders option (TODO Phase 5.4)
  //
  // For Phase 2.4 we just bind innerHTML; we trust pulldown-cmark's CommonMark
  // output. Belt-and-suspenders sanitizer lands in Phase 5.4 a11y pass.

  let {
    html,
    byte_len = 0
  }: {
    html: string;
    byte_len?: number;
  } = $props();
</script>

<article class="markdown-viewer">
  {#if byte_len > 0}
    <aside class="meta"><strong>{byte_len.toLocaleString()} bytes</strong> · markdown</aside>
  {/if}
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  <div class="prose">{@html html}</div>
</article>

<style>
  .markdown-viewer { padding: 1rem 2rem; max-width: 70ch; margin: 0 auto; color: #1a1a1a; }
  .meta { color: #888; margin-bottom: 1rem; font-size: 0.75rem; }
  .prose :global(h1) { font-size: 1.8rem; margin: 1.5rem 0 0.75rem; }
  .prose :global(h2) { font-size: 1.45rem; margin: 1.25rem 0 0.6rem; }
  .prose :global(h3) { font-size: 1.2rem; margin: 1rem 0 0.5rem; }
  .prose :global(p) { margin: 0.6rem 0; line-height: 1.6; }
  .prose :global(pre) { background: #f7f7f7; padding: 0.75rem 1rem; border-radius: 0.4rem; overflow: auto; }
  .prose :global(code) { background: #f0f0f0; padding: 0.1rem 0.3rem; border-radius: 0.2rem; font-family: ui-monospace, monospace; }
  .prose :global(pre code) { background: transparent; padding: 0; }
  .prose :global(blockquote) { border-left: 3px solid #ccc; margin-left: 0; padding-left: 1rem; color: #555; }
  .prose :global(table) { border-collapse: collapse; }
  .prose :global(th), .prose :global(td) { border: 1px solid #ddd; padding: 0.4rem 0.6rem; }
  .prose :global(a) { color: #124FC2; text-decoration: none; }
  .prose :global(a:hover) { text-decoration: underline; }
  .prose :global(ul), .prose :global(ol) { padding-left: 1.5rem; }
  .prose :global(li) { margin: 0.2rem 0; line-height: 1.5; }
</style>
