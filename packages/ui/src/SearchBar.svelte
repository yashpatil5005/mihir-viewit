<script lang="ts">
  // Phase 5.2 — Search bar used by text-bearing viewers.
  let {
    onSearch = (q: string, cs: boolean) => {},
    matchCount = 0,
    currentMatch = 0,
    onNext = () => {},
    onPrev = () => {},
  }: {
    onSearch: (q: string, cs: boolean) => void;
    matchCount?: number;
    currentMatch?: number;
    onNext?: () => void;
    onPrev?: () => void;
  } = $props();

  let query = $state("");
  let caseSensitive = $state(false);

  $effect(() => {
    onSearch(query, caseSensitive);
  });
</script>

<div class="search-bar">
  <input
    type="search"
    placeholder="Search in file…"
    bind:value={query}
    aria-label="Search in file"
  />
  <button
    class="cs-toggle"
    class:active={caseSensitive}
    onclick={() => (caseSensitive = !caseSensitive)}
    aria-label="Case sensitive"
    title="Case sensitive">Aa</button
  >
  {#if query}
    <span class="count">{currentMatch}/{matchCount}</span>
    <button onclick={onPrev} aria-label="Previous match">↑</button>
    <button onclick={onNext} aria-label="Next match">↓</button>
  {/if}
</div>

<style>
  .search-bar {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    padding: 0.4rem 1rem;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
  }
  input[type="search"] {
    flex: 1;
    padding: 0.3rem 0.5rem;
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    font-size: 0.85rem;
  }
  button {
    cursor: pointer;
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    padding: 0.3rem 0.6rem;
    font-size: 0.85rem;
  }
  .cs-toggle.active {
    background: var(--border);
  }
  .count {
    font-size: 0.75rem;
    color: var(--text-secondary);
    min-width: 3.5rem;
    text-align: center;
  }
</style>
