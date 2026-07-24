<script lang="ts">
  import {
    formatPluginSize,
    hasAndroidBridge,
    installPlugin,
    pluginInventory,
    removePlugin,
    type PluginInfo,
  } from './pluginBridge';

  let {
    open = false,
    onClose,
  }: {
    open: boolean;
    onClose: () => void;
  } = $props();

  let catalog = $state<PluginInfo[]>([]);
  let installed = $state<PluginInfo[]>([]);
  let loading = $state(true);
  let installing = $state<string | null>(null);
  let installProgress = $state(0);
  let errorMsg = $state('');

  const isAndroid = $derived(hasAndroidBridge());

  async function refresh() {
    loading = true;
    errorMsg = '';
    try {
      if (!isAndroid) {
        errorMsg = 'Plugin store is only available on Android';
        loading = false;
        return;
      }
      const inventory = await pluginInventory();
      installed = inventory.installed;
      catalog = inventory.catalog;
    } catch (e) {
      errorMsg = `Failed to load plugins: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  async function install(plugin: PluginInfo) {
    if (!isAndroid) return;
    installing = plugin.id;
    installProgress = 0;
    errorMsg = '';

    try {
      await installPlugin(plugin);
      await new Promise(r => setTimeout(r, 3000));
      await refresh();
    } catch (e) {
      errorMsg = `Install failed: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      installing = null;
    }
  }

  async function remove(pluginId: string) {
    if (!isAndroid) return;
    try {
      await removePlugin(pluginId);
      await refresh();
    } catch (e) {
      errorMsg = `Remove failed: ${e instanceof Error ? e.message : String(e)}`;
    }
  }

  $effect(() => {
    if (open) refresh();
  });
</script>

{#if open}
<div class="overlay" onclick={onClose} role="presentation">
  <div
    class="modal"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
    role="dialog"
    tabindex="-1"
  >
    <header>
      <h2>Plugin Store</h2>
      <button class="close-btn" onclick={onClose}>&times;</button>
    </header>

    {#if loading}
      <div class="loading">Loading plugins...</div>
    {:else if errorMsg}
      <div class="error">{errorMsg}</div>
    {:else}
      {#if installed.length > 0}
        <section>
          <h3>Installed</h3>
          {#each installed as plugin}
            <div class="plugin-card installed">
              <div class="plugin-info">
                <strong>{plugin.name}</strong>
                <span class="version">v{plugin.version}</span>
                <span class="formats">{plugin.formats.join(', ')}</span>
              </div>
              <button class="remove-btn" onclick={() => remove(plugin.id)}>Remove</button>
            </div>
          {/each}
        </section>
      {/if}

      <section>
        <h3>Available</h3>
        {#if catalog.length === 0}
          <div class="empty">No plugins available yet.</div>
        {:else}
          {#each catalog as plugin}
            <div class="plugin-card">
              <div class="plugin-info">
                <strong>{plugin.name}</strong>
                <span class="version">v{plugin.version}</span>
                <p class="description">{plugin.description}</p>
                <span class="formats">{plugin.formats.join(', ')}</span>
                <span class="size">{formatPluginSize(plugin.sizeBytes)} download{plugin.installedSizeBytes ? ` · ${formatPluginSize(plugin.installedSizeBytes)} installed` : ''}</span>
              </div>
              {#if plugin.installed}
                <span class="installed-badge">Installed</span>
              {:else if installing === plugin.id}
                <div class="progress">
                  <div class="progress-bar" style="width: {installProgress * 100}%"></div>
                  <span>Installing...</span>
                </div>
              {:else}
                <button class="install-btn" onclick={() => install(plugin)}>Install</button>
              {/if}
            </div>
          {/each}
        {/if}
      </section>
    {/if}
  </div>
</div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .modal {
    background: var(--bg-primary, #fff);
    color: var(--text-primary, #000);
    border-radius: 12px;
    width: min(90vw, 480px);
    max-height: 80vh;
    overflow-y: auto;
    padding: 1.5rem;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid var(--border, #ccc);
  }
  h2 { margin: 0; font-size: 1.2rem; }
  h3 { margin: 0.75rem 0 0.5rem; font-size: 0.9rem; color: var(--text-secondary, #666); text-transform: uppercase; letter-spacing: 0.05em; }
  .close-btn {
    background: none; border: none; font-size: 1.5rem; cursor: pointer;
    color: var(--text-secondary, #666); padding: 0.25rem;
  }
  .loading, .empty { text-align: center; padding: 2rem; color: var(--text-secondary, #666); }
  .error { color: var(--error, #e53e3e); padding: 0.75rem; background: var(--bg-secondary, #f5f5f5); border-radius: 8px; margin-bottom: 1rem; font-size: 0.85rem; }
  .plugin-card {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.75rem; margin-bottom: 0.5rem;
    border: 1px solid var(--border, #ccc); border-radius: 8px;
    gap: 0.75rem;
  }
  .plugin-card.installed { border-color: var(--link, #3182ce); background: var(--bg-secondary, #f0f7ff); }
  .plugin-info { flex: 1; min-width: 0; }
  .plugin-info strong { display: block; font-size: 0.95rem; }
  .version { font-size: 0.75rem; color: var(--text-secondary, #666); }
  .description { font-size: 0.8rem; margin: 0.25rem 0; color: var(--text-secondary, #666); }
  .formats { display: block; font-size: 0.7rem; font-family: monospace; color: var(--text-secondary, #888); margin-top: 0.25rem; }
  .size { font-size: 0.7rem; color: var(--text-secondary, #888); }
  .install-btn, .remove-btn {
    padding: 0.4rem 1rem; border-radius: 6px; font-size: 0.8rem;
    font-weight: 600; cursor: pointer; white-space: nowrap;
  }
  .install-btn { background: var(--link, #3182ce); color: #fff; border: none; }
  .install-btn:hover { opacity: 0.9; }
  .remove-btn { background: none; color: var(--error, #e53e3e); border: 1px solid var(--error, #e53e3e); }
  .installed-badge { font-size: 0.75rem; color: var(--link, #3182ce); font-weight: 600; }
  .progress { display: flex; flex-direction: column; align-items: center; gap: 0.25rem; }
  .progress-bar { height: 4px; background: var(--link, #3182ce); border-radius: 2px; width: 100%; }
</style>
