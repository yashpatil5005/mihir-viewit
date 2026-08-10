<script lang="ts">
  // Formats without a built-in handler are provided by downloadable plugins.
  // Instead of a dead "scaffold" message, tell the user a plugin adds this
  // format and let them install it (plugins enrich/watch the basic experience).
  let {
    document: docProp = {},
    onInstall = null,
  }: {
    document?: { format?: string; name?: string; byte_len?: number };
    onInstall?: (() => void) | null;
  } = $props();

  let format = $derived(docProp.format ?? "unsupported");
  let name = $derived(docProp.name ?? "file");
  let byte_len = $derived(docProp.byte_len ?? 0);

  const OFFICE = new Set([
    "docx",
    "docm",
    "dotx",
    "dotm",
    "xlsx",
    "xlsm",
    "xlsb",
    "xls",
    "pptx",
    "pptm",
    "potx",
    "odt",
    "ott",
    "ods",
    "ots",
    "odp",
    "otp",
    "doc",
    "ppt",
  ]);
  const ARCHIVE = new Set([
    "zip",
    "7z",
    "rar",
    "tar",
    "gz",
    "tgz",
    "bz2",
    "tbz2",
    "xz",
    "txz",
    "zst",
    "tzst",
    "lz4",
    "lzma",
    "tlz",
    "tar.gz",
    "tar.bz2",
    "tar.xz",
    "tar.zst",
    "tar.lz4",
    "tar.lzma",
  ]);
  const FONT = new Set(["ttf", "otf", "woff", "woff2", "ttc", "pfb", "cff", "dfont", "sfd", "ps"]);

  let pluginName = $derived.by(() => {
    const f = format.toLowerCase();
    if (OFFICE.has(f)) return "Office";
    if (ARCHIVE.has(f)) return "Archive";
    if (FONT.has(f)) return "Font";
    return null;
  });

  let online = $derived(typeof navigator !== "undefined" ? navigator.onLine : true);
</script>

<article class="placeholder">
  <aside class="meta">
    <strong>{name}</strong> — {byte_len.toLocaleString()} bytes
  </aside>

  {#if pluginName}
    <p class="lead">This format is provided by the <strong>{pluginName}</strong> ViewIt plugin.</p>
    <p>
      Install it to preview <code>{format}</code> — plugins enrich the built-in viewer and work fully
      offline once installed.
    </p>
    {#if online && onInstall}
      <button type="button" onclick={onInstall}>Install the {pluginName} plugin</button>
    {:else if online}
      <p class="hint">Go to Plugin Store to install the {pluginName} plugin.</p>
    {:else}
      <p class="hint">
        You're offline. Install the {pluginName} plugin when you're back online to preview this format.
      </p>
    {/if}
  {:else}
    <p>The <code>{format}</code> format has no viewer built in.</p>
  {/if}
</article>

<style>
  .placeholder {
    padding: 1rem;
    color: var(--text-secondary);
  }
  .meta {
    color: var(--text-secondary);
    margin-bottom: 0.5rem;
    font-size: 0.85rem;
  }
  .lead {
    margin: 0.2rem 0;
    color: var(--text-primary);
  }
  p {
    line-height: 1.5;
    margin: 0.4rem 0;
  }
  button {
    margin-top: 0.6rem;
    padding: 0.5rem 0.9rem;
    border: 1px solid var(--link);
    border-radius: 0.5rem;
    background: var(--link);
    color: #fff;
    cursor: pointer;
    font-weight: 600;
  }
  .hint {
    font-size: 0.85rem;
    font-style: italic;
  }
  code {
    background: var(--bg-secondary);
    padding: 0.1rem 0.3rem;
    border-radius: 0.2rem;
  }
</style>
