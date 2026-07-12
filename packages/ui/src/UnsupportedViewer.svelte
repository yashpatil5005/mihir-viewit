<script lang="ts">
  import { openWithExternal } from '@viewit/platform';
  let {
    format,
    reason,
    suggestion,
    uri
  }: {
    format: string;
    reason: string;
    suggestion?: 'open-with-external' | 'none';
    uri?: string;
  } = $props();
</script>

<article class="unsupported">
  <h2>Can't open this file</h2>
  <p><code>{format}</code></p>
  <p class="reason">{reason}</p>
  {#if suggestion === 'open-with-external' && uri}
    <button onclick={() => openWithExternal(uri)}>Open with another app…</button>
  {/if}
</article>

<style>
  .unsupported { padding: 2rem; text-align: center; color: #555; }
  .reason { margin-bottom: 1rem; }
  button { padding: 0.5rem 1rem; cursor: pointer; }
</style>
