# ViewIt Plugin Bases (architecture)

The app ships a small built-in viewer capable of a **basic offline experience**
for a large matrix of formats. Downloadable **plugins enrich it** — or take over
whole experiences. A **base** is the kind of experience a plugin provides; it lets
the WebView route work to the right add-on instead of hard-coding per plugin.

## Principles

1. The built-in viewer (fmt-*) always provides **something** for a format (basic
   text/structure, offline). It is never a dead "Phase 1 scaffold" message.
2. A plugin **enriches** (richer rendering — e.g. office-universal bundling
   pptx-vanilla) or **takes over** a base when it declares that base.
3. Plugins are **downloadable**, never baked into the APK, and work fully offline
   after install. The APK stays under its size budget.

## Bases

| Base    | Meaning                                                        | Example |
|---------|----------------------------------------------------------------|---------|
| `view`  | Add/enrich a viewer for formats (default).                     | office-universal (pptx-vanilla for `.pptx`) |
| `play`  | Playback experience — a **player base** can swap the built-in native media player with a custom UI/engine. | a future `player` plugin |
| `edit`  | Transform/save experience; editing is the plugin's responsibility (round-trip the file). | a future `editor` plugin |
| `tool`  | Utility for a capability (extract, transcode, …) reachable from a viewer. | compression-universal (extract), ffmpeg-transcoder |

## Declaring a base

Each plugin manifest carries:

```json
{
  "id": "…",
  "base": "play",            // default "view"
  "runtime": "js" | "native",
  "capabilities": [ "…" ],
  "supportedFormats": [ "…" ]
}
```

`base` is optional (defaults to `view`). It flows from the catalog →
`PluginManifest` (Kotlin) → `PluginInfo` (TS), so the WebView can route by base.

## Routing

- Open a format → built-in viewer shows basic offline result.
- If a plugin whose `supportedFormats` covers the format (and whose base applies:
  `view`/`play` for display, `edit` when the user edits, `tool` for an action) is
  installed → prefer the plugin's experience.
- Missing plugin + online → one-tap **Install & reopen** (see `Viewer.svelte`).
- Missing plugin + offline → clear message + install once back online.

## Current status

- `base` descriptor: added to `PluginManifest`, catalog, `PluginInfo`.
- Today every shipped plugin is `view` (or `tool`-like via capabilities). The
  **player base** and **editor base** are the natural next plugin families once
  the download pipeline is stable (see `docs/` for release/deploy flow).
