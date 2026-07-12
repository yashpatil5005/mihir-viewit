<script lang="ts">
  let { content, encoding = 'utf-8', byte_len = 0 }: { content: string; encoding?: string; byte_len?: number } = $props();
</script>

<article class="text-viewer">
  {#if byte_len > 0}
    <aside class="meta">
      <strong>{byte_len.toLocaleString()} bytes</strong> · encoding: <code>{encoding}</code>
    </aside>
  {/if}
  <pre class="content">{content}</pre>
</article>

<style>
  .text-viewer { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.875rem; }
  .meta { font-family: system-ui, sans-serif; color: #888; margin-bottom: 0.5rem; font-size: 0.75rem; }
  .content { padding: 0.75rem 1rem; background: #f7f7f7; border-radius: 0.4rem; overflow: auto; white-space: pre-wrap; word-break: break-word; }
</style>
