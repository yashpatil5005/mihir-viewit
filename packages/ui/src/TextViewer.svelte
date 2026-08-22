<script lang="ts">
  import SearchBar from "./SearchBar.svelte";
  import { fullscreenState } from "./fullscreen.svelte";
  import { findAllMatches, escapeHtml, type Match } from "./search";

  let {
    content,
    encoding = "utf-8",
    byte_len = 0,
    truncated = false,
  }: { content: string; encoding?: string; byte_len?: number; truncated?: boolean } = $props();

  const CHUNK_LINES = 400;
  const CHUNKS_PER_FRAME = 6;

  type Chunk = { text: string; start: number };

  // Split the document into fixed-size line chunks in a single pass. Chunks
  // are the unit of progressive mounting, so opening a multi-megabyte file
  // never blocks the main thread with one enormous DOM/text node.
  //
  // NOTE: `content` must be snapshotted to a local before any per-character
  // loop — the Svelte compiler lowers destructured props to `$$props.content`
  // proxy reads, and millions of proxy traps freeze the main thread.
  let chunks = $derived.by(() => {
    const text = content;
    const out: Chunk[] = [];
    if (text.length === 0) return out;
    let chunkStart = 0;
    let linesInChunk = 0;
    let pos = 0;
    while (pos <= text.length) {
      if (pos === text.length || text.charCodeAt(pos) === 10) {
        linesInChunk++;
        if (linesInChunk === CHUNK_LINES || pos === text.length) {
          out.push({ text: text.slice(chunkStart, pos), start: chunkStart });
          chunkStart = pos + 1;
          linesInChunk = 0;
        }
        if (pos === text.length) break;
      }
      pos++;
    }
    return out;
  });

  let lineCount = $derived.by(() => {
    const text = content;
    let n = 1;
    for (let i = 0; i < text.length; i++) if (text.charCodeAt(i) === 10) n++;
    return n;
  });

  let mountedCount = $state(1);
  let fullyMounted = $derived(mountedCount >= chunks.length);
  let scheduled = false;

  function pump() {
    scheduled = false;
    if (mountedCount >= chunks.length) return;
    const next = Math.min(chunks.length, mountedCount + CHUNKS_PER_FRAME);
    mountedCount = next;
    if (mountedCount < chunks.length) schedule();
  }

  function schedule() {
    if (scheduled) return;
    scheduled = true;
    if (typeof requestAnimationFrame === "function") requestAnimationFrame(pump);
    else setTimeout(pump, 16);
  }

  $effect(() => {
    if (chunks.length > mountedCount) schedule();
  });

  function ensureMountedUpTo(charIndex: number) {
    // Binary search for the chunk containing charIndex and mount through it
    // immediately so search navigation never waits on the background ramp.
    let lo = 0;
    let hi = chunks.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (chunks[mid].start <= charIndex) lo = mid;
      else hi = mid - 1;
    }
    if (lo + 1 > mountedCount) mountedCount = lo + 1;
  }

  let query = $state("");
  let caseSensitive = $state(false);
  let matches: Match[] = $state([]);
  let currentIdx = $state(0);
  let contentEl: HTMLElement | null = $state(null);

  // Bumped whenever highlighting inputs change, so cached per-chunk HTML can
  // be invalidated cheaply without recomputing untouched documents.
  let highlightVersion = 0;

  function runSearch(q: string, cs: boolean) {
    // The SearchBar effect can fire with unchanged inputs whenever this
    // component re-renders (its onSearch closure is recreated). Assigning
    // fresh match arrays on every fire would invalidate state forever, so
    // ignore no-op searches.
    if (q === query && cs === caseSensitive) return;
    caseSensitive = cs;
    query = q;
    matches = findAllMatches(content, q, cs);
    currentIdx = 0;
    highlightVersion++;
    scrollToMatch(0);
  }

  // Rendered HTML per mounted chunk, computed in a single derived so the
  // each-block re-renders deterministically when mounting ramps up or the
  // search changes. The memo keeps the background ramp O(new chunks).
  let chunkCache = new Map<number, { v: number; html: string }>();
  let renderedChunks = $derived.by(() => {
    const count = Math.min(mountedCount, chunks.length);
    const out: string[] = [];
    for (let i = 0; i < count; i++) {
      const chunk = chunks[i];
      const hit = chunkCache.get(chunk.start);
      if (hit && hit.v === highlightVersion) {
        out.push(hit.html);
        continue;
      }
      const html = renderChunkHtml(chunk, i);
      chunkCache.set(chunk.start, { v: highlightVersion, html });
      out.push(html);
    }
    return out;
  });

  function scrollToMatch(i: number) {
    if (i < 0 || i >= matches.length) return;
    currentIdx = i;
    ensureMountedUpTo(matches[i].index);
    tryScrollToMark(i, 120);
  }

  // The target match's chunk may still be mounting (or Svelte may not have
  // flushed yet), so retry across frames until its mark exists in the DOM.
  function tryScrollToMark(i: number, attempts: number) {
    const mark = contentEl?.querySelectorAll("mark")[i];
    if (mark) {
      mark.scrollIntoView({ block: "center", behavior: "smooth" });
      return;
    }
    if (attempts > 0) {
      if (typeof requestAnimationFrame === "function")
        requestAnimationFrame(() => tryScrollToMark(i, attempts - 1));
      else setTimeout(() => tryScrollToMark(i, attempts - 1), 16);
    }
  }

  // Highlight one chunk independently. Matches are sorted by index, so a
  // binary search finds the first match that can reach this chunk and the
  // scan stops at the chunk end — O(window), not O(all matches). Matches
  // spanning a chunk boundary are split into adjacent marks, which renders
  // identically to whole-document highlighting.
  function renderChunkHtml(chunk: Chunk, chunkIndex: number): string {
    const chunkEnd = chunkIndex + 1 < chunks.length ? chunks[chunkIndex + 1].start : content.length;
    if (!query) return escapeHtml(chunk.text);
    const parts: string[] = [];
    let cursor = chunk.start;
    let lo = 0;
    let hi = matches.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (matches[mid].index + matches[mid].length <= chunk.start) lo = mid + 1;
      else hi = mid;
    }
    for (let i = lo; i < matches.length; i++) {
      const m = matches[i];
      if (m.index >= chunkEnd) break;
      const s = Math.max(m.index, cursor);
      const e = Math.min(m.index + m.length, chunkEnd);
      if (e <= s) continue;
      parts.push(escapeHtml(chunk.text.slice(cursor - chunk.start, s - chunk.start)));
      parts.push(
        "<mark>" + escapeHtml(chunk.text.slice(s - chunk.start, e - chunk.start)) + "</mark>",
      );
      cursor = e;
    }
    parts.push(escapeHtml(chunk.text.slice(cursor - chunk.start)));
    return parts.join("");
  }
</script>

<article class="text-viewer" class:fs={fullscreenState.active}>
  <SearchBar
    onSearch={runSearch}
    matchCount={matches.length}
    currentMatch={currentIdx + 1}
    onNext={() => scrollToMatch(currentIdx + 1)}
    onPrev={() => scrollToMatch(currentIdx - 1)}
  />
  {#if byte_len > 0}
    <aside class="meta">
      <strong>{byte_len.toLocaleString()} bytes</strong> · {lineCount.toLocaleString()} lines · encoding:
      <code>{encoding}</code>{#if truncated}
        · <em>truncated preview — load full file to view all</em>{/if}
    </aside>
  {/if}
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  <pre class="content" bind:this={contentEl}>
    {#each renderedChunks as htmlStr, i (chunks[i].start)}
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      <code class="chunk"
        >{@html htmlStr}</code
      >
    {/each}
    {#if !fullyMounted}
      <span class="mounting">loading remaining lines…</span>
    {/if}
  </pre>
</article>

<style>
  .text-viewer {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.875rem;
  }
  .text-viewer.fs .meta {
    display: none;
  }
  .meta {
    font-family: system-ui, sans-serif;
    color: var(--text-secondary);
    margin-bottom: 0.5rem;
    font-size: 0.75rem;
  }
  .content {
    padding: 0.75rem 1rem;
    margin: 0;
    background: var(--bg-secondary);
    color: var(--text-primary);
    border-radius: 0.4rem;
    white-space: pre-wrap;
    word-break: break-word;
    max-width: 100%;
  }
  /* Offscreen chunks skip layout and paint entirely; intrinsic size keeps the
     scrollbar stable while they are skipped. */
  .chunk {
    display: block;
    content-visibility: auto;
    contain-intrinsic-size: auto 26rem;
  }
  .mounting {
    color: var(--text-secondary);
    font-style: italic;
    font-family: system-ui, sans-serif;
    font-size: 0.75rem;
  }
  .content :global(mark) {
    background: rgba(255, 213, 79, 0.6);
    border-radius: 0.15rem;
  }
</style>
