<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    open = $bindable(false),
    onClose = () => {},
  }: {
    open: boolean;
    onClose?: () => void;
  } = $props();

  const SETTINGS_KEY = "omnia.settings";

  export interface AppSettings {
    enableMediaPreviews: boolean;
    searchInsideArchives: boolean;
    maxSearchResults: number;
    searchModeDefault: "auto" | "filename" | "content";
  }

  const defaultSettings: AppSettings = {
    enableMediaPreviews: false, // Default off to preserve RAM and prevent OOM
    searchInsideArchives: false,
    maxSearchResults: 50,
    searchModeDefault: "auto",
  };

  let settings = $state<AppSettings>(loadSettings());

  function loadSettings(): AppSettings {
    if (typeof localStorage === "undefined") return defaultSettings;
    try {
      const raw = localStorage.getItem(SETTINGS_KEY);
      return raw ? { ...defaultSettings, ...JSON.parse(raw) } : defaultSettings;
    } catch {
      return defaultSettings;
    }
  }

  function saveSettings() {
    if (typeof localStorage === "undefined") return;
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
    } catch {
      // ignore
    }
  }

  function togglePreviews() {
    settings.enableMediaPreviews = !settings.enableMediaPreviews;
    saveSettings();
  }

  function toggleArchives() {
    settings.searchInsideArchives = !settings.searchInsideArchives;
    saveSettings();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && open) {
      open = false;
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <div class="settings-backdrop" role="dialog" aria-modal="true" aria-label="Settings">
    <div class="settings-modal">
      <div class="settings-header">
        <div class="title-group">
          <Icon name="sliders" size={20} />
          <h2>Settings & Preferences</h2>
        </div>
        <button
          type="button"
          class="close-btn"
          onclick={() => {
            open = false;
            onClose();
          }}
          aria-label="Close settings"
        >
          <Icon name="x" size={18} />
        </button>
      </div>

      <div class="settings-body">
        <div class="setting-item">
          <div class="setting-text">
            <strong>Media Previews in Browse Grid</strong>
            <p>
              Generate live image & video thumbnail posters. Disabling this saves significant RAM
              and prevents low-memory slowdowns.
            </p>
          </div>
          <button
            type="button"
            class="toggle-btn"
            class:active={settings.enableMediaPreviews}
            onclick={togglePreviews}
            aria-pressed={settings.enableMediaPreviews}
          >
            {settings.enableMediaPreviews ? "Enabled" : "Disabled"}
          </button>
        </div>

        <div class="setting-item">
          <div class="setting-text">
            <strong>Deep Document & Content Search</strong>
            <p>
              Stream scan inside text, Markdown, code, DOCX, and XLSX using minimal memory ring
              buffers.
            </p>
          </div>
          <span class="badge-active">Active (Rust Stream)</span>
        </div>

        <div class="setting-item">
          <div class="setting-text">
            <strong>Default Search Mode</strong>
            <p>Choose search priority when typing into the global SearchBar.</p>
          </div>
          <select
            bind:value={settings.searchModeDefault}
            onchange={saveSettings}
            class="select-input"
          >
            <option value="auto">Auto (Fuzzy Filename + Content fallback)</option>
            <option value="filename">Filename Only</option>
            <option value="content">Content Stream Only</option>
          </select>
        </div>
      </div>

      <div class="settings-footer">
        <button
          type="button"
          class="done-btn"
          onclick={() => {
            open = false;
            onClose();
          }}
        >
          Done
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .settings-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(4px);
    display: grid;
    place-items: center;
    z-index: 1000;
    padding: 1rem;
  }
  .settings-modal {
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: 0.8rem;
    max-width: 32rem;
    width: 100%;
    overflow: hidden;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
  }
  .settings-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.2rem;
    border-bottom: 1px solid var(--border);
  }
  .title-group {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
  .title-group h2 {
    font-size: 1.1rem;
    margin: 0;
    font-weight: 700;
  }
  .close-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-secondary);
    padding: 0.4rem;
    display: grid;
    place-items: center;
    border-radius: 0.3rem;
  }
  .close-btn:hover {
    color: var(--text-primary);
    background: var(--bg-secondary);
  }
  .settings-body {
    padding: 1.2rem;
    display: flex;
    flex-direction: column;
    gap: 1.2rem;
  }
  .setting-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }
  .setting-text strong {
    display: block;
    font-size: 0.95rem;
    margin-bottom: 0.2rem;
  }
  .setting-text p {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-secondary);
    line-height: 1.4;
  }
  .toggle-btn {
    padding: 0.4rem 0.8rem;
    border-radius: 0.4rem;
    border: 1px solid var(--border);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 0.85rem;
    font-weight: 600;
    min-width: 80px;
  }
  .toggle-btn.active {
    background: var(--link);
    color: #fff;
    border-color: var(--link);
  }
  .badge-active {
    font-size: 0.75rem;
    font-weight: 600;
    color: #10b981;
    background: rgba(16, 185, 129, 0.1);
    padding: 0.3rem 0.6rem;
    border-radius: 0.3rem;
    white-space: nowrap;
  }
  .select-input {
    background: var(--bg-secondary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.4rem 0.6rem;
    font-size: 0.85rem;
  }
  .settings-footer {
    padding: 0.8rem 1.2rem;
    border-top: 1px solid var(--border);
    background: var(--bg-secondary);
    display: flex;
    justify-content: flex-end;
  }
  .done-btn {
    padding: 0.45rem 1.2rem;
    border-radius: 0.4rem;
    background: var(--text-primary);
    color: var(--bg-primary);
    border: none;
    font-weight: 600;
    cursor: pointer;
  }
</style>
