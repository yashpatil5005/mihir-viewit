<script lang="ts">
  import { openWithExternal } from "@viewit/platform";
  import { tick } from "svelte";
  import Icon from "./Icon.svelte";
  import {
    fetchPluginCatalogSources,
    formatPluginSize,
    hasAndroidBridge,
    installPlugin,
    isInstallablePlugin,
    listInstalledPlugins,
    pluginSupports,
    type PluginInfo,
  } from "./pluginBridge";
  import { providersForPackages, resolveProvider } from "@viewit/contracts/resolver";

  let {
    open = false,
    uri = "",
    name = "file",
    ext = "",
    builtInLabel = "Built-in viewer",
    service = "viewit.document.parse",
    onClose,
    onUseBuiltIn,
    onUseInstalledPlugin,
  }: {
    open: boolean;
    uri: string;
    name?: string;
    ext: string;
    builtInLabel?: string;
    service?: `viewit.${string}`;
    onClose: () => void;
    onUseBuiltIn: () => void | Promise<void>;
    onUseInstalledPlugin?: (plugin: PluginInfo) => void | Promise<void>;
  } = $props();

  let loading = $state(false);
  let installing = $state<string | null>(null);
  let installed = $state<PluginInfo[]>([]);
  let downloadable = $state<PluginInfo[]>([]);
  let errorMsg = $state("");
  let chooserEl = $state<HTMLElement | null>(null);
  let previousFocus: HTMLElement | null = null;

  async function refresh() {
    if (!open) return;
    loading = true;
    errorMsg = "";
    try {
      const allInstalled = await listInstalledPlugins();
      const catalog = (await fetchPluginCatalogSources()).flatMap((source) => source.plugins);
      const installedDecision = resolveProvider(
        { service, contractVersion: 1, format: ext },
        providersForPackages(
          allInstalled.map((plugin) => plugin.id),
          "installed",
        ),
      );
      const downloadableDecision = resolveProvider(
        { service, contractVersion: 1, format: ext },
        providersForPackages(
          catalog.map((plugin) => plugin.id),
          "downloadable",
        ),
      );
      const installedIds = new Set(
        installedDecision.candidates.map((provider) => provider.packageId),
      );
      const downloadableIds = new Set(
        downloadableDecision.candidates.map((provider) => provider.packageId),
      );
      installed = allInstalled.filter(
        (plugin) => installedIds.has(plugin.id) && pluginSupports(plugin, ext),
      );
      downloadable = catalog
        .filter(
          (plugin) =>
            downloadableIds.has(plugin.id) &&
            pluginSupports(plugin, ext) &&
            isInstallablePlugin(plugin),
        )
        .map((plugin) => ({
          ...plugin,
          installed: allInstalled.some((installedPlugin) => installedPlugin.id === plugin.id),
        }));
    } catch (e) {
      errorMsg = `Could not load plugins: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  async function installAndRefresh(plugin: PluginInfo) {
    installing = plugin.id;
    errorMsg = "";
    try {
      await installPlugin(plugin);
      await new Promise((resolve) => setTimeout(resolve, 3000));
      await refresh();
    } catch (e) {
      errorMsg = `Install failed: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      installing = null;
    }
  }

  async function chooseInstalled(plugin: PluginInfo) {
    await onUseInstalledPlugin?.(plugin);
    onClose();
  }

  async function chooseBuiltIn() {
    await onUseBuiltIn();
    onClose();
  }

  $effect(() => {
    if (open) {
      previousFocus = document.activeElement as HTMLElement | null;
      void refresh();
      void tick().then(() => chooserEl?.focus());
    } else if (previousFocus) {
      previousFocus.focus();
      previousFocus = null;
    }
  });

  function handleDialogKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  }
</script>

{#if open}
  <div class="overlay" onclick={onClose} role="presentation">
    <div
      class="chooser"
      bind:this={chooserEl}
      role="dialog"
      aria-modal="true"
      aria-labelledby="runtime-title"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={handleDialogKeydown}
    >
      <header>
        <div>
          <p class="eyebrow">Runtime</p>
          <h2 id="runtime-title">Open {name}</h2>
        </div>
        <button class="close" type="button" onclick={onClose} aria-label="Close runtime chooser"
          ><Icon name="x" /></button
        >
      </header>

      <p class="summary">
        Choose how ViewIt should handle <code>.{ext}</code>. Plugin sizes are optional downloads,
        not part of the base app.
      </p>

      <button class="option primary" type="button" onclick={chooseBuiltIn}>
        <strong>{builtInLabel}</strong>
        <span>Smallest path. Uses ViewIt's built-in runtime first.</span>
      </button>

      {#if hasAndroidBridge()}
        {#if loading}
          <p class="muted" role="status" aria-live="polite">
            Checking installed and downloadable plugins…
          </p>
        {:else if errorMsg}
          <p class="error" role="alert">{errorMsg}</p>
        {:else}
          {#each installed as plugin}
            <button class="option" type="button" onclick={() => chooseInstalled(plugin)}>
              <strong>{plugin.name}</strong>
              <span>Installed plugin · {plugin.formats.join(", ")}</span>
            </button>
          {/each}

          {#each downloadable as plugin}
            {#if !plugin.installed}
              <div class="option download">
                <div>
                  <strong>{plugin.name}</strong>
                  <span>{plugin.description}</span>
                  {#if plugin.sourceName}<small>{plugin.sourceName}</small>{/if}
                  <small
                    >{formatPluginSize(plugin.sizeBytes)} download{plugin.installedSizeBytes
                      ? ` · ${formatPluginSize(plugin.installedSizeBytes)} installed`
                      : ""}</small
                  >
                </div>
                <button
                  type="button"
                  onclick={() => installAndRefresh(plugin)}
                  disabled={installing === plugin.id}
                >
                  {installing === plugin.id ? "Installing…" : "Install"}
                </button>
              </div>
            {/if}
          {/each}

          {#if installed.length === 0 && downloadable.filter((plugin) => !plugin.installed).length === 0}
            <p class="muted">No plugin in the current catalog handles <code>.{ext}</code>.</p>
          {/if}
        {/if}
      {/if}

      <button
        class="option external"
        type="button"
        onclick={() => {
          openWithExternal(uri);
          onClose();
        }}
      >
        <strong>Open with another app</strong>
        <span>Use Android or the host OS if ViewIt cannot handle this file.</span>
      </button>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 1100;
    background: rgba(0, 0, 0, 0.58);
    display: grid;
    place-items: center;
    padding: max(1rem, env(safe-area-inset-top, 0px)) max(1rem, env(safe-area-inset-right, 0px))
      max(1rem, env(safe-area-inset-bottom, 0px)) max(1rem, env(safe-area-inset-left, 0px));
  }
  .chooser {
    width: min(94vw, 34rem);
    max-height: 86vh;
    max-height: 86dvh;
    overflow: auto;
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 1rem;
    box-shadow: 0 1.5rem 4rem rgba(0, 0, 0, 0.35);
    padding: 1rem;
  }
  header {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    align-items: start;
    margin-bottom: 0.75rem;
  }
  .eyebrow {
    margin: 0 0 0.2rem;
    color: var(--text-secondary);
    font-size: 0.72rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  h2 {
    margin: 0;
    font-size: 1.15rem;
  }
  .close {
    border: 0;
    background: transparent;
    color: var(--text-secondary);
    font-size: 1.5rem;
    cursor: pointer;
    min-width: 44px;
    min-height: 44px;
  }
  .summary,
  .muted {
    color: var(--text-secondary);
    font-size: 0.86rem;
    line-height: 1.45;
  }
  code {
    background: var(--bg-secondary);
    padding: 0.08rem 0.24rem;
    border-radius: 0.25rem;
  }
  .option {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    text-align: left;
    border: 1px solid var(--border);
    background: var(--bg-secondary);
    color: var(--text-primary);
    border-radius: 0.75rem;
    padding: 0.8rem;
    margin: 0.55rem 0;
    cursor: pointer;
  }
  .option strong {
    display: block;
    font-size: 0.94rem;
  }
  .option span,
  .option small {
    display: block;
    color: var(--text-secondary);
    font-size: 0.78rem;
    line-height: 1.35;
    margin-top: 0.16rem;
  }
  .primary {
    border-color: var(--link);
    background: color-mix(in srgb, var(--link) 10%, var(--bg-primary));
  }
  .external {
    border-style: dashed;
  }
  .download {
    cursor: default;
  }
  .download > div,
  .option > span {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .download button {
    flex: 0 0 auto;
    border: 0;
    border-radius: 0.55rem;
    padding: 0.48rem 0.8rem;
    background: var(--link);
    color: white;
    font-weight: 700;
    cursor: pointer;
    min-height: 44px;
  }
  .download button:disabled {
    opacity: 0.65;
    cursor: wait;
  }
  .error {
    color: var(--error);
    background: var(--bg-secondary);
    border-radius: 0.6rem;
    padding: 0.65rem;
    font-size: 0.82rem;
  }
  @media (max-width: 520px) {
    .download {
      align-items: stretch;
      flex-direction: column;
    }
    .download button {
      width: 100%;
    }
  }
</style>
