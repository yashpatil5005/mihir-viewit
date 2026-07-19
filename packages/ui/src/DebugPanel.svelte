<script lang="ts">
  import { debugLogLines, debugLogClear } from '@viewit/platform';

  let { open = $bindable(false) }: { open?: boolean } = $props();
  let text = $state('');

  $effect(() => {
    if (!open) return;
    const tick = () => {
      text = debugLogLines().join('\n') || '(no events yet — open a file)';
    };
    tick();
    const id = setInterval(tick, 500);
    return () => clearInterval(id);
  });
</script>

{#if open}
  <aside class="debug-panel" aria-label="Debug log">
    <header>
      <strong>Debug</strong> (no Chrome needed)
      <button type="button" onclick={() => debugLogClear()}>Clear</button>
      <button type="button" onclick={() => (open = false)}>✕</button>
    </header>
    <pre>{text}</pre>
  </aside>
{/if}

<style>
  .debug-panel {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    max-height: 40vh;
    z-index: 100;
    background: #111;
    color: #8f8;
    font-size: 0.65rem;
    border-top: 2px solid #484;
    display: flex;
    flex-direction: column;
  }
  header {
    display: flex;
    gap: 0.5rem;
    padding: 0.35rem 0.5rem;
    background: #222;
    align-items: center;
  }
  header button { font-size: 0.65rem; padding: 0.2rem 0.4rem; }
  pre {
    margin: 0;
    padding: 0.5rem;
    overflow: auto;
    flex: 1;
    white-space: pre-wrap;
    word-break: break-all;
  }
</style>