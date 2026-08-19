<script lang="ts">
  // Phase 5.9 — Onboarding / first-run hint. Dimissable, persists in localStorage.
  let { onClose = () => {} }: { onClose?: () => void } = $props();
  const STORAGE_KEY = "viewit-onboarded";
  let visible = $state(false);

  if (typeof localStorage !== "undefined" && !localStorage.getItem(STORAGE_KEY)) {
    visible = true;
  }

  function dismiss() {
    visible = false;
    if (typeof localStorage !== "undefined") localStorage.setItem(STORAGE_KEY, "1");
    onClose();
  }
</script>

{#if visible}
  <section class="onboard" aria-labelledby="onboard-title">
    <p class="eyebrow">Private by default</p>
    <h2 id="onboard-title">Your files, clearly opened.</h2>
    <p>View documents, media, images, books, and archives without uploading them.</p>
    <p class="hint">Choose <strong>Open file</strong>, drop a file here, or use Browse files.</p>
    <button onclick={dismiss} aria-label="Dismiss introduction">Continue</button>
  </section>
{/if}

<style>
  .onboard {
    padding: clamp(1.4rem, 4vw, 2.25rem);
    background: linear-gradient(145deg, var(--bg-secondary), var(--bg-primary));
    border: 1px solid var(--border);
    border-radius: 1rem;
    margin: clamp(1rem, 8vh, 5rem) auto 1rem;
    max-width: 42rem;
    color: var(--text-primary);
  }
  h2 {
    max-width: 13ch;
    font-size: clamp(1.8rem, 5vw, 3.5rem);
    line-height: 1.02;
    letter-spacing: -0.045em;
    margin: 0 0 1rem;
  }
  p {
    max-width: 34rem;
    font-size: 1rem;
    line-height: 1.55;
    margin: 0.45rem 0;
  }
  .eyebrow {
    color: var(--link);
    font-size: 0.72rem;
    font-weight: 750;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  .hint {
    color: var(--text-secondary);
  }
  button {
    cursor: pointer;
    margin-top: 0.6rem;
    min-height: 44px;
    padding: 0.55rem 1rem;
    background: var(--text-primary);
    color: var(--bg-primary);
    border: 1px solid var(--text-primary);
    border-radius: 0.5rem;
    font-size: 0.9rem;
    font-weight: 650;
  }
</style>
