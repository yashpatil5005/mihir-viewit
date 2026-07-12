<script lang="ts">
  let { format, name, byte_len }: { format: string; name: string; byte_len: number } = $props();
</script>

<article class="placeholder">
  <aside class="meta">
    <strong>{name}</strong> — {byte_len.toLocaleString()} bytes
  </aside>
  <p>Phase 1 scaffold — the <code>{format}</code> handler lands in a later phase.</p>
  <p>Pick a <code>.txt</code> file to see the actual open-render loop in action.</p>
</article>

<style>
  .placeholder { padding: 1rem; color: #555; }
  .meta { color: #888; margin-bottom: 0.5rem; font-size: 0.85rem; }
  code { background: #f0f0f0; padding: 0.1rem 0.3rem; border-radius: 0.2rem; }
</style>
