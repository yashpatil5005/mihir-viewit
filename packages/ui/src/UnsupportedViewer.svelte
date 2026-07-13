<script lang="ts">
  import { openWithExternal } from '@viewit/platform';
  let {
    document: docProp = {},
    uri
  }: {
    document?: { format?: string; reason?: string; suggestion?: string };
    uri?: string;
  } = $props();

  const format = docProp.format ?? 'unsupported';
  const reason = docProp.reason ?? '';
  const suggestion = (docProp.suggestion as 'open-with-external' | 'none' | undefined) ?? 'none';
</script>

<article class="unsupported">
  <div class="icon" aria-hidden="true">🔓</div>
  <h2>Can't open this file inside ViewIt</h2>
  <p><code>{format}</code></p>
  <p class="reason">{reason}</p>
  {#if suggestion === 'open-with-external' && uri}
    <button class="cta" onclick={() => openWithExternal(uri)} aria-label="Open with another app">Open with another app…</button>
  {/if}
</article>

<style>
  .unsupported { padding: 2rem; text-align: center; color: var(--text-secondary); }
  .icon { font-size: 3rem; opacity: 0.6; margin-bottom: 0.5rem; }
  h2 { color: var(--text-primary); margin: 0 0 0.5rem; }
  .reason { margin-bottom: 1.5rem; max-width: 400px; margin-left: auto; margin-right: auto; line-height: 1.4; }
  .cta { padding: 0.6rem 1.4rem; cursor: pointer; background: var(--link); color: #fff; border: none; border-radius: 0.4rem; font-size: 0.95rem; }
  .cta:hover { opacity: 0.9; }
</style>
