<script lang="ts">
  import {
    formatPluginSize,
    fetchPluginCatalogSources,
    getCustomCatalogUrls,
    hasAndroidBridge,
    installPlugin,
    isInstallablePlugin,
    isRestartToApplyError,
    removePlugin,
    restartApp,
    saveCustomCatalogUrls,
    type PluginCatalogSource,
    type PluginInfo,
  } from "./pluginBridge";

  let {
    open = false,
    onClose,
  }: {
    open: boolean;
    onClose: () => void;
  } = $props();

  let catalog = $state<PluginInfo[]>([]);
  let catalogSources = $state<PluginCatalogSource[]>([]);
  let installed = $state<PluginInfo[]>([]);
  let loading = $state(true);
  let installing = $state<string | null>(null);
  let installProgress = $state(0);
  let errorMsg = $state("");
  let restartRequired = $state(false);
  let showPrivacy = $state(false);
  let acceptedPrivacy = $state(false);
  let customCatalogUrl = $state("");
  let wasOpen = $state(false);

  const isAndroid = $derived(hasAndroidBridge());

  const PRIVACY_TEXT = `Plugin Privacy & Security

• Plugins are optional downloadable runtimes
• Plugins run in a sandboxed process with no network access
• Plugin code is verified via SHA-256 checksum against the signed catalog
• No personal data is sent to plugin authors
• Plugins cannot access files outside the document being viewed
• Plugin failures fall back to the built-in viewer without crashing`;

  function acceptPrivacy() {
    acceptedPrivacy = true;
    showPrivacy = false;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("viewit-plugin-privacy-accepted", "1");
    }
  }

  async function refresh() {
    loading = true;
    errorMsg = "";
    try {
      if (!isAndroid) {
        errorMsg = "Plugin store is only available on Android";
        loading = false;
        return;
      }
      installed = JSON.parse((window as any).AndroidBridge.listPlugins()) as PluginInfo[];
      const installedById = new Map(installed.map((plugin) => [plugin.id, plugin]));
      catalogSources = (await fetchPluginCatalogSources()).map((source) => ({
        ...source,
        plugins: source.plugins.filter(isInstallablePlugin).map((plugin) => {
          const installedPlugin = installedById.get(plugin.id);
          const installedVersion = installedPlugin?.version;
          const versionMatches = installedVersion === plugin.version;
          return {
            ...plugin,
            installed: Boolean(installedPlugin && versionMatches),
            installedVersion,
            updateAvailable: Boolean(installedPlugin && !versionMatches),
          };
        }),
      }));
      catalog = catalogSources.flatMap((source) => source.plugins);
      if (
        typeof localStorage !== "undefined" &&
        localStorage.getItem("viewit-plugin-privacy-accepted") === "1"
      ) {
        acceptedPrivacy = true;
      }
    } catch (e) {
      errorMsg = `Failed to load plugins: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  async function install(plugin: PluginInfo) {
    if (!isAndroid || !acceptedPrivacy) return;
    installing = plugin.id;
    installProgress = 0;
    errorMsg = "";

    try {
      restartRequired = false;
      await installPlugin(plugin);
      await new Promise((r) => setTimeout(r, 3000));
      await refresh();
    } catch (e) {
      if (isRestartToApplyError(e)) {
        restartRequired = true;
        errorMsg = "Update downloaded. A restart is needed to apply it.";
      } else {
        errorMsg = `Install failed: ${e instanceof Error ? e.message : String(e)}`;
      }
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

  async function addCustomCatalog() {
    const url = customCatalogUrl.trim();
    if (!url) return;
    if (!/^https?:\/\//i.test(url)) {
      errorMsg = "Catalog URL must start with http:// or https://";
      return;
    }
    saveCustomCatalogUrls([...getCustomCatalogUrls(), url]);
    customCatalogUrl = "";
    await refresh();
  }

  async function removeCustomCatalog(url: string) {
    saveCustomCatalogUrls(getCustomCatalogUrls().filter((item) => item !== url));
    await refresh();
  }

  $effect(() => {
    if (open && !wasOpen) {
      wasOpen = true;
      void refresh();
    } else if (!open) {
      wasOpen = false;
    }
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

      {#if !acceptedPrivacy}
        <section class="privacy-section">
          <h3>Plugin Privacy & Security</h3>
          <pre>{PRIVACY_TEXT}</pre>
          <div class="privacy-actions">
            <button class="accept-btn" onclick={acceptPrivacy}>I Understand</button>
          </div>
        </section>
      {:else if loading}
        <div class="loading">Loading plugins...</div>
      {:else if errorMsg}
        <div class="error">{errorMsg}</div>
        {#if restartRequired}
          <div class="restart-cta">
            <button type="button" onclick={restartApp}>Restart app to apply</button>
          </div>
        {/if}
      {:else}
        <section>
          <h3>Plugins</h3>
          {#if catalog.length === 0}
            <div class="empty">No plugins available yet.</div>
          {:else}
            {#each catalogSources as source}
              <div class="source-heading">
                <div>
                  <strong>{source.name}</strong>
                  {#if source.url}<span>Custom catalog</span>{:else}<span>Default catalog</span
                    >{/if}
                </div>
                {#if source.url}<button
                    class="remove-source"
                    type="button"
                    onclick={() => removeCustomCatalog(source.url!)}>Remove</button
                  >{/if}
              </div>
              {#if source.error}
                <div class="error">Could not load catalog: {source.error}</div>
              {:else if source.plugins.length === 0}
                <div class="empty compact">No installable plugins in this catalog.</div>
              {/if}
              {#each source.plugins as plugin}
                <div
                  class="plugin-card"
                  class:installed={plugin.installed}
                  class:update={plugin.updateAvailable}
                >
                  <div class="plugin-info">
                    <strong>{plugin.name}</strong>
                    <span class="version">
                      Catalog v{plugin.version}{#if plugin.installedVersion && plugin.installedVersion !== plugin.version}
                        · installed v{plugin.installedVersion}{/if}
                    </span>
                    <p class="description">{plugin.description}</p>
                    {#if (plugin as unknown as { base?: string }).base && (plugin as unknown as { base?: string }).base !== "view"}
                      <span class="base-badge">base: {(plugin as unknown as { base?: string }).base}</span>
                    {/if}
                    {#if plugin.capabilities && plugin.capabilities.length > 0}
                      <span class="caps">{plugin.capabilities.join(" · ")}</span>
                    {/if}
                    <span class="formats">{plugin.formats.join(", ")}</span>
                    <span class="size"
                      >{formatPluginSize(plugin.sizeBytes)} download{plugin.installedSizeBytes
                        ? ` · ${formatPluginSize(plugin.installedSizeBytes)} installed`
                        : ""}</span
                    >
                    {#if plugin.storageScope}<p class="scope">{plugin.storageScope}</p>{/if}
                    {#if plugin.sourceUrl}<span class="source-url">From {plugin.sourceUrl}</span
                      >{/if}
                  </div>
                  {#if installing === plugin.id}
                    <div class="progress">
                      <div class="progress-bar" style="width: {installProgress * 100}%"></div>
                      <span>Installing...</span>
                    </div>
                  {:else if plugin.updateAvailable}
                    <div class="plugin-actions">
                      <button class="install-btn" onclick={() => install(plugin)}>Update</button>
                      <button class="remove-btn" onclick={() => remove(plugin.id)}>Remove</button>
                    </div>
                  {:else if plugin.installed}
                    <div class="plugin-actions">
                      <span class="installed-badge">Installed</span>
                      <button class="remove-btn" onclick={() => remove(plugin.id)}>Remove</button>
                    </div>
                  {:else}
                    <button class="install-btn" onclick={() => install(plugin)}>Install</button>
                  {/if}
                </div>
              {/each}
            {/each}
          {/if}
        </section>

        <section class="custom-catalogs">
          <h3>Add Custom Catalog</h3>
          <p>Custom public catalog URLs are shown separately from the ViewIt default catalog.</p>
          <div class="catalog-form">
            <input bind:value={customCatalogUrl} placeholder="https://example.com/catalog.json" />
            <button type="button" onclick={addCustomCatalog}>Add</button>
          </div>
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
  h2 {
    margin: 0;
    font-size: 1.2rem;
  }
  h3 {
    margin: 0.75rem 0 0.5rem;
    font-size: 0.9rem;
    color: var(--text-secondary, #666);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .close-btn {
    background: none;
    border: none;
    font-size: 1.5rem;
    cursor: pointer;
    color: var(--text-secondary, #666);
    padding: 0.25rem;
  }
  .loading,
  .empty {
    text-align: center;
    padding: 2rem;
    color: var(--text-secondary, #666);
  }
  .empty.compact {
    padding: 0.75rem;
    font-size: 0.8rem;
  }
  .error {
    color: var(--error, #e53e3e);
    padding: 0.75rem;
    background: var(--bg-secondary, #f5f5f5);
    border-radius: 8px;
    margin-bottom: 1rem;
    font-size: 0.85rem;
  }
  .restart-cta {
    margin-bottom: 1rem;
  }
  .restart-cta button {
    padding: 0.5rem 0.9rem;
    border: 1px solid var(--link);
    border-radius: 0.5rem;
    background: var(--link);
    color: #fff;
    cursor: pointer;
    font-weight: 600;
  }
  .plugin-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem;
    margin-bottom: 0.5rem;
    border: 1px solid var(--border, #ccc);
    border-radius: 8px;
    gap: 0.75rem;
  }
  .plugin-card.installed {
    border-color: var(--link, #3182ce);
    background: var(--bg-secondary, #f0f7ff);
  }
  .plugin-info {
    flex: 1;
    min-width: 0;
  }
  .plugin-info strong {
    display: block;
    font-size: 0.95rem;
  }
  .version {
    font-size: 0.75rem;
    color: var(--text-secondary, #666);
  }
  .description {
    font-size: 0.8rem;
    margin: 0.25rem 0;
    color: var(--text-secondary, #666);
  }
  .formats {
    display: block;
    font-size: 0.7rem;
    font-family: monospace;
    color: var(--text-secondary, #888);
    margin-top: 0.25rem;
  }
  .size {
    font-size: 0.7rem;
    color: var(--text-secondary, #888);
  }
  .source-url {
    display: block;
    margin-top: 0.25rem;
    font-size: 0.68rem;
    color: var(--text-secondary, #888);
    overflow-wrap: anywhere;
  }
  .scope {
    display: block;
    margin-top: 0.35rem;
    font-size: 0.7rem;
    color: var(--text-secondary, #888);
    overflow-wrap: anywhere;
    border-top: 1px dashed var(--border, #ddd);
    padding-top: 0.3rem;
  }
  .source-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.75rem;
    margin: 1rem 0 0.5rem;
    padding-top: 0.75rem;
    border-top: 1px solid var(--border, #ccc);
  }
  .source-heading strong {
    display: block;
    font-size: 0.84rem;
    overflow-wrap: anywhere;
  }
  .source-heading span {
    display: block;
    color: var(--text-secondary, #666);
    font-size: 0.72rem;
  }
  .remove-source {
    border: 1px solid var(--error, #e53e3e);
    color: var(--error, #e53e3e);
    background: transparent;
    border-radius: 6px;
    padding: 0.3rem 0.55rem;
    cursor: pointer;
  }
  .install-btn,
  .remove-btn {
    padding: 0.4rem 1rem;
    border-radius: 6px;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
  }
  .install-btn {
    background: var(--link, #3182ce);
    color: #fff;
    border: none;
  }
  .install-btn:hover {
    opacity: 0.9;
  }
  .remove-btn {
    background: none;
    color: var(--error, #e53e3e);
    border: 1px solid var(--error, #e53e3e);
  }
  .installed-badge {
    font-size: 0.75rem;
    color: var(--link, #3182ce);
    font-weight: 600;
  }
  .plugin-card.update {
    border-color: #d97706;
    background: color-mix(in srgb, #f59e0b 12%, var(--bg-primary, #fff));
  }
  .plugin-actions {
    display: flex;
    gap: 0.45rem;
    align-items: center;
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .progress {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
  }
  .progress-bar {
    height: 4px;
    background: var(--link, #3182ce);
    border-radius: 2px;
    width: 100%;
  }
  .privacy-section {
    padding: 1rem 0;
  }
  .privacy-section h3 {
    margin: 0 0 0.75rem;
    font-size: 1rem;
    color: var(--text-primary, #000);
  }
  .privacy-section pre {
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    line-height: 1.6;
    white-space: pre-wrap;
    color: var(--text-secondary, #666);
    background: var(--bg-secondary, #f5f5f5);
    padding: 0.75rem;
    border-radius: 6px;
    margin: 0 0 1rem;
  }
  .privacy-actions {
    display: flex;
    justify-content: flex-end;
  }
  .accept-btn {
    padding: 0.5rem 1.5rem;
    border-radius: 6px;
    font-size: 0.9rem;
    font-weight: 600;
    cursor: pointer;
    background: var(--link, #3182ce);
    color: #fff;
    border: none;
  }
  .custom-catalogs {
    margin-top: 1.25rem;
    padding-top: 1rem;
    border-top: 1px solid var(--border, #ccc);
  }
  .custom-catalogs p {
    margin: 0 0 0.7rem;
    color: var(--text-secondary, #666);
    font-size: 0.78rem;
    line-height: 1.4;
  }
  .catalog-form {
    display: flex;
    gap: 0.5rem;
  }
  .catalog-form input {
    flex: 1;
    min-width: 0;
    border: 1px solid var(--border, #ccc);
    background: var(--bg-secondary, #f5f5f5);
    color: var(--text-primary, #000);
    border-radius: 6px;
    padding: 0.48rem 0.6rem;
  }
  .catalog-form button {
    border: 0;
    border-radius: 6px;
    padding: 0.48rem 0.8rem;
    background: var(--link, #3182ce);
    color: white;
    font-weight: 700;
    cursor: pointer;
  }
  .base-badge {
    display: inline-block;
    padding: 0.1rem 0.4rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--link) 22%, transparent);
    color: var(--link);
    font-size: 0.72rem;
    font-weight: 700;
  }
  .caps {
    margin-left: 0.35rem;
    color: var(--text-secondary);
    font-size: 0.75rem;
  }
</style>
