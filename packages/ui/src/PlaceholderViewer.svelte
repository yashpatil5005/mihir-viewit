<script lang="ts">
  let { document: docProp = {} }: { document?: { format?: string; name?: string; byte_len?: number } } = $props();
  const format = docProp.format ?? 'unsupported';
  const name = docProp.name ?? 'file';
  const byte_len = docProp.byte_len ?? 0;
</script>

<article class="placeholder">
  <aside class="meta">
    <strong>{name}</strong> — {byte_len.toLocaleString()} bytes
  </aside>
  <p>Phase 1 scaffold — the <code>{format}</code> handler lands in a later phase.</p>
  <p>Pick a <code>.txt</code> file to see the actual open-render loop in action.</p>
</article>

<style>
  .placeholder { padding: 1rem; color: var(--text-secondary); }
  .meta { color: var(--text-secondary); margin-bottom: 0.5rem; font-size: 0.85rem; }
  code { background: var(--bg-secondary); padding: 0.1rem 0.3rem; border-radius: 0.2rem; }
</style>
