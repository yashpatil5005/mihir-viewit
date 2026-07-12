<script lang="ts">
  // Phase 2.5 — Pretty-printed JSON rendered as plain pre text. The Rust
  // side already re-serialized via serde_json::to_string_pretty; the
  // frontend just displays it.

  let {
    pretty,
    byte_len = 0
  }: {
    pretty: string;
    byte_len?: number;
  } = $props();
</script>

<article class="json-viewer">
  {#if byte_len > 0}
    <aside class="meta"><strong>{byte_len.toLocaleString()} bytes</strong> · json</aside>
  {/if}
  <pre class="pretty">{pretty}</pre>
</article>

<style>
  .json-viewer { padding: 0.5rem 1rem; }
  .meta { color: #888; margin-bottom: 0.5rem; font-size: 0.75rem; }
  .pretty { background: #f7f7f7; padding: 0.75rem 1rem; border-radius: 0.4rem;
            font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
            font-size: 0.85rem; overflow: auto; }
</style>
