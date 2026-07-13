<script lang="ts">
  // Phase 5.9 — Onboarding / first-run hint. Dimissable, persists in localStorage.
  let { onClose = () => {} }: { onClose?: () => void } = $props();
  const STORAGE_KEY = 'viewit-onboarded';
  let visible = $state(false);

  if (typeof localStorage !== 'undefined' && !localStorage.getItem(STORAGE_KEY)) {
    visible = true;
  }

  function dismiss() {
    visible = false;
    if (typeof localStorage !== 'undefined') localStorage.setItem(STORAGE_KEY, '1');
    onClose();
  }
</script>

{#if visible}
  <div class="onboard" role="dialog" aria-labelledby="onboard-title">
    <h2 id="onboard-title">Open any file</h2>
    <p>Tap <strong>Open file…</strong> or <strong>▦</strong> to browse. Everything opens — text, images, PDFs, Office docs, archives.</p>
    <p><strong>☾</strong> toggles dark mode.</p>
    <button onclick={dismiss} aria-label="Dismiss onboarding">Got it</button>
  </div>
{/if}

<style>
  .onboard { padding: 1rem 1.5rem; background: var(--bg-secondary); border: 1px solid var(--border); border-radius: 0.5rem; margin: 1rem auto; max-width: 480px; text-align: center; color: var(--text-primary); }
  h2 { font-size: 1.1rem; margin: 0 0 0.5rem; }
  p { font-size: 0.85rem; line-height: 1.4; margin: 0.4rem 0; }
  button { cursor: pointer; margin-top: 0.6rem; padding: 0.4rem 1rem; background: var(--link); color: #fff; border: none; border-radius: 0.3rem; font-size: 0.85rem; }
</style>