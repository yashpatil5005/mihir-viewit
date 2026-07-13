<script lang="ts">
  import SearchBar from './SearchBar.svelte';
  import { findAllMatches, highlightHtml, type Match } from './search';

  let { content, encoding = 'utf-8', byte_len = 0 }: { content: string; encoding?: string; byte_len?: number } = $props();

  let query = $state('');
  let caseSensitive = $state(false);
  let matches: Match[] = [];
  let currentIdx = $state(0);
  let contentEl: HTMLElement | null = $state(null);

  function runSearch(q: string, cs: boolean) {
    caseSensitive = cs;
    matches = findAllMatches(content, q, cs);
    currentIdx = 0;
    scrollToMatch(0);
  }

  function scrollToMatch(i: number) {
    if (i < 0 || i >= matches.length) return;
    currentIdx = i;
    const mark = contentEl?.querySelectorAll('mark')[i];
    if (mark) mark.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }

  let displayed = $derived.by(() => {
    if (!query) return highlightHtml(content, []);
    return highlightHtml(content, matches);
  });
</script>

<article class="text-viewer">
  <SearchBar onSearch={runSearch} matchCount={matches.length} currentMatch={currentIdx + 1} onNext={() => scrollToMatch(currentIdx + 1)} onPrev={() => scrollToMatch(currentIdx - 1)} />
  {#if byte_len > 0}
    <aside class="meta">
      <strong>{byte_len.toLocaleString()} bytes</strong> · encoding: <code>{encoding}</code>
    </aside>
  {/if}
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  <pre class="content" bind:this={contentEl}>{@html displayed}</pre>
</article>

<style>
  .text-viewer { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.875rem; }
  .meta { font-family: system-ui, sans-serif; color: var(--text-secondary); margin-bottom: 0.5rem; font-size: 0.75rem; }
  .content { padding: 0.75rem 1rem; background: var(--bg-secondary); color: var(--text-primary); border-radius: 0.4rem; overflow: auto; white-space: pre-wrap; word-break: break-word; }
  .content :global(mark) { background: rgba(255, 213, 79, 0.6); border-radius: 0.15rem; }
  .content :global(mark:nth-child(1)) { outline: 2px solid var(--link); }
</style>