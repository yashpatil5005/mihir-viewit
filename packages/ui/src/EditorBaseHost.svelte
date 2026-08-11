<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { loadJsPlugin, saveEditedText, type PluginInfo } from "./pluginBridge";

  let {
    text = "",
    name = "document.txt",
    mime = "text/plain",
    plugin = null,
    onClose = null,
  }: {
    text?: string;
    name?: string;
    mime?: string;
    plugin?: PluginInfo | null;
    onClose?: (() => void) | null;
  } = $props();

  let hostEl: HTMLDivElement | null = $state(null);
  let status = $state<"loading" | "ready" | "saving" | "saved" | "error">("loading");
  let errorMsg = $state("");
  let editor: any = null;

  onMount(async () => {
    try {
      if (!plugin) throw new Error("No editor-base plugin selected");
      const mod = await loadJsPlugin(plugin);
      if (!mod || typeof mod.createEditor !== "function")
        throw new Error(`${plugin.name} has no createEditor export`);
      if (!hostEl) throw new Error("Editor host not ready");
      editor = mod.createEditor(hostEl, { text, name });
      status = "ready";
    } catch (e) {
      status = "error";
      errorMsg = e instanceof Error ? e.message : String(e);
    }
  });

  async function save() {
    if (!editor) return;
    status = "saving";
    try {
      await saveEditedText(editor.getText(), name, mime);
      status = "saved";
      setTimeout(() => {
        if (status === "saved") status = "ready";
      }, 1800);
    } catch (e) {
      status = "error";
      errorMsg = e instanceof Error ? e.message : String(e);
    }
  }

  onDestroy(() => {
    try {
      editor?.destroy?.();
    } catch {}
    editor = null;
  });
</script>

<div class="editor-base-host">
  <div class="editor-toolbar">
    <strong>{plugin?.name ?? "Editor"}</strong>
    <span>{status === "saving" ? "Saving…" : status === "saved" ? "Saved" : ""}</span>
    <button type="button" onclick={save} disabled={status === "saving" || status === "loading"}
      >Save as…</button
    >
    {#if onClose}<button type="button" class="close" onclick={onClose}>Close editor</button>{/if}
  </div>
  {#if errorMsg}<p class="error">{errorMsg}</p>{/if}
  <div class="editor-host" class:busy={status === "loading"} bind:this={hostEl}></div>
</div>

<style>
  .editor-base-host {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    height: 100%;
    min-height: 45vh;
  }
  .editor-toolbar {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    font-size: 0.8rem;
  }
  .editor-toolbar span {
    color: var(--text-secondary);
    margin-left: auto;
  }
  .editor-toolbar button {
    padding: 0.35rem 0.65rem;
    border: 1px solid var(--link);
    border-radius: 0.4rem;
    background: var(--link);
    color: #fff;
    cursor: pointer;
  }
  .editor-toolbar button.close {
    background: transparent;
    color: var(--text-primary);
    border-color: var(--border);
  }
  .editor-host {
    flex: 1;
  }
  .editor-host.busy {
    visibility: hidden;
  }
  .error {
    color: var(--danger, #e5484d);
  }
</style>
