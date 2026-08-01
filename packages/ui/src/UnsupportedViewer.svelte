<script lang="ts">
  import { openWithExternal } from '@viewit/platform';
  import {
    fetchPluginCatalogSources,
    pluginSupports,
    isInstallablePlugin,
    installPlugin,
    formatPluginSize,
    type PluginInfo,
    type PluginCatalogSource,
  } from './pluginBridge';

  let {
    document: docProp = {},
    uri,
    onPluginInstalled,
  }: {
    document?: { format?: string; reason?: string; suggestion?: string };
    uri?: string;
    onPluginInstalled?: () => void;
  } = $props();

  let format = $derived(docProp.format ?? 'unsupported');
  let reason = $derived(docProp.reason ?? '');
  let suggestion = $derived((docProp.suggestion as 'open-with-external' | 'none' | undefined) ?? 'none');

  let suggestedPlugins = $state<PluginInfo[]>([]);
  let installingId = $state<string | null>(null);
  let installError = $state<string | null>(null);
  let loaded = $state(false);

  const ext = $derived(format.replace(/^\./, '').toLowerCase());

  $effect(() => {
    if (loaded || !ext || ext === 'unsupported') return;
    loaded = true;
    (async () => {
      try {
        const sources = await fetchPluginCatalogSources();
        const all = sources.flatMap((s) => s.plugins);
        const matching = all.filter((p) => pluginSupports(p, ext) && isInstallablePlugin(p));
        const seen = new Set<string>();
        suggestedPlugins = matching.filter((p) => {
          if (seen.has(p.id)) return false;
          seen.add(p.id);
          return true;
        });
      } catch {
        suggestedPlugins = [];
      }
    })();
  });

  async function handleInstall(plugin: PluginInfo) {
    installingId = plugin.id;
    installError = null;
    try {
      await installPlugin(plugin);
      installingId = null;
      onPluginInstalled?.();
    } catch (e) {
      installError = e instanceof Error ? e.message : String(e);
      installingId = null;
    }
  }
</script>

<article class="unsupported">
  <div class="icon" aria-hidden="true">🔓</div>
  <h2>Can't open this file inside ViewIt</h2>
  <p><code>{format}</code></p>
  <p class="reason">{reason}</p>

  {#if suggestedPlugins.length > 0}
    <div class="plugins-section">
      <p class="plugins-header">A plugin can handle this format:</p>
      {#each suggestedPlugins as plugin}
        <div class="plugin-card">
          <div class="plugin-info">
            <strong>{plugin.name}</strong>
            <span class="plugin-desc">{plugin.description}</span>
            <span class="plugin-formats">{plugin.formats.join(', ')}</span>
            <span class="plugin-size">{formatPluginSize(plugin.sizeBytes)} download · {formatPluginSize(plugin.installedSizeBytes)} installed</span>
          </div>
          {#if installingId === plugin.id}
            <span class="plugin-status">Installing…</span>
          {:else}
            <button class="install-btn" onclick={() => handleInstall(plugin)}>
              Install
            </button>
          {/if}
        </div>
      {/each}
      {#if installError}
        <p class="install-error">{installError}</p>
      {/if}
    </div>
  {/if}

  {#if suggestion === 'open-with-external' && uri}
    <button class="cta" onclick={() => openWithExternal(uri)} aria-label="Open with another app">Open with another app…</button>
  {/if}
</article>

<style>
  .unsupported { padding: 2rem; text-align: center; color: var(--text-secondary); }
  .icon { font-size: 3rem; opacity: 0.6; margin-bottom: 0.5rem; }
  h2 { color: var(--text-primary); margin: 0 0 0.5rem; }
  .reason { margin-bottom: 1.5rem; max-width: 400px; margin-left: auto; margin-right: auto; line-height: 1.4; }
  .cta { padding: 0.6rem 1.4rem; cursor: pointer; background: var(--link); color: #fff; border: none; border-radius: 0.4rem; font-size: 0.95rem; margin-top: 0.5rem; }
  .cta:hover { opacity: 0.9; }

  .plugins-section { margin: 1.5rem auto; max-width: 420px; text-align: left; }
  .plugins-header { font-weight: 600; color: var(--text-primary); margin-bottom: 0.75rem; text-align: center; }
  .plugin-card {
    display: flex; align-items: center; justify-content: space-between;
    gap: 0.75rem; padding: 0.75rem 1rem; margin-bottom: 0.5rem;
    border: 1px solid var(--border); border-radius: 0.5rem;
    background: var(--bg-secondary);
  }
  .plugin-info { display: flex; flex-direction: column; gap: 0.15rem; min-width: 0; }
  .plugin-info strong { color: var(--text-primary); }
  .plugin-desc { font-size: 0.8rem; color: var(--text-secondary); }
  .plugin-formats { font-size: 0.75rem; font-family: ui-monospace, monospace; color: var(--text-secondary); }
  .plugin-size { font-size: 0.7rem; color: var(--text-secondary); opacity: 0.7; }
  .plugin-status { font-size: 0.8rem; color: var(--text-secondary); font-style: italic; white-space: nowrap; }
  .install-btn {
    padding: 0.4rem 1rem; cursor: pointer; background: var(--link); color: #fff;
    border: none; border-radius: 0.3rem; font-size: 0.85rem; white-space: nowrap; flex-shrink: 0;
  }
  .install-btn:hover { opacity: 0.9; }
  .install-error { color: var(--error); font-size: 0.8rem; margin-top: 0.5rem; text-align: center; }
</style>
