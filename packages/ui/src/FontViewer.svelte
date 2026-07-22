<script lang="ts">
  let {
    family_name = 'Unknown',
    weight = 400,
    is_italic = false,
    format = 'font',
    byte_len = 0,
  }: {
    family_name?: string;
    weight?: number;
    is_italic?: boolean;
    format?: string;
    byte_len?: number;
  } = $props();

  const weightLabel: Record<number, string> = {
    100: 'Thin', 200: 'ExtraLight', 300: 'Light', 400: 'Regular',
    500: 'Medium', 600: 'SemiBold', 700: 'Bold', 800: 'ExtraBold', 900: 'Black',
  };
</script>

<article class="font-viewer">
  <div class="preview">
    <p class="sample" style="font-weight: {weight}; font-style: {is_italic ? 'italic' : 'normal'}">
      The quick brown fox jumps over the lazy dog
    </p>
    <p class="sample-lg" style="font-weight: {weight}; font-style: {is_italic ? 'italic' : 'normal'}">
      Aa Bb Cc 0123
    </p>
  </div>
  <aside class="meta">
    <strong>{family_name}</strong>
    <span class="tag">{format}</span>
    <span class="detail">{weightLabel[weight] ?? weight}{is_italic ? ' Italic' : ''}</span>
    <span class="detail">{(byte_len / 1024).toFixed(0)} KB</span>
  </aside>
</article>

<style>
  .font-viewer { display: flex; flex-direction: column; height: 100%; }
  .preview { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 1.5rem; padding: 2rem; background: var(--bg-secondary); }
  .sample { font-size: 1.1rem; color: var(--text-primary); margin: 0; }
  .sample-lg { font-size: 2.5rem; color: var(--text-primary); margin: 0; letter-spacing: 0.05em; }
  .meta { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: baseline; padding: 0.75rem 1rem; border-top: 1px solid var(--border); font-size: 0.8rem; color: var(--text-secondary); }
  .meta strong { color: var(--text-primary); }
  .tag { font-family: ui-monospace, monospace; background: var(--bg-secondary); padding: 0.1rem 0.4rem; border-radius: 0.25rem; }
  .detail { font-size: 0.75rem; opacity: 0.85; }
</style>
