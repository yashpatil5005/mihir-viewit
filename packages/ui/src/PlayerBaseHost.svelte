<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { loadJsPlugin, type PluginInfo } from "./pluginBridge";

  let {
    source = "",
    kind = "video",
    name = "",
    plugin = null,
  }: {
    source?: string;
    kind?: "video" | "audio";
    name?: string;
    plugin?: PluginInfo | null;
  } = $props();

  let hostEl: HTMLDivElement | null = $state(null);
  let status = $state<"loading" | "ready" | "error">("loading");
  let errorMsg = $state("");
  let player: any = null;

  onMount(async () => {
    if (!plugin) {
      status = "error";
      errorMsg = "No player-base plugin selected";
      return;
    }
    try {
      const mod = await loadJsPlugin(plugin);
      if (!mod || typeof mod.createPlayer !== "function") {
        throw new Error(`${plugin.name} has no createPlayer export`);
      }
      if (!hostEl) throw new Error("Player host not ready");
      player = mod.createPlayer(hostEl, { source: source || undefined, kind, name });
      status = "ready";
    } catch (e) {
      status = "error";
      errorMsg = e instanceof Error ? e.message : String(e);
    }
  });

  onDestroy(() => {
    try {
      player?.destroy?.();
    } catch {
      /* ignore */
    }
    player = null;
  });
</script>

<div class="player-base">
  {#if status === "loading"}<p class="status">Loading custom player…</p>{/if}
  {#if status === "error"}<p class="status error">{errorMsg}</p>{/if}
  <div class="player-host" class:busy={status !== "ready"} bind:this={hostEl}></div>
</div>

<style>
  .player-base {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    height: 100%;
  }
  .status {
    color: var(--text-secondary);
    font-size: 0.8rem;
  }
  .error {
    color: var(--danger, #e5484d);
  }
  .player-host {
    flex: 1;
    min-height: 200px;
    border-radius: 0.6rem;
    overflow: hidden;
    background: #000;
  }
  .player-host.busy {
    visibility: hidden;
  }
</style>
