# Architecture

> High-level view of how a file flows from user choice to on-screen render,
> across the three targets (desktop, Android, web). See also
> [`../STREAMING-ARCHITECTURE.md`](../STREAMING-ARCHITECTURE.md) for the byte
> transport details and [`../PLUGIN-ARCHITECTURE.md`](../PLUGIN-ARCHITECTURE.md)
> for the plugin catalog model.

## Flow

```mermaid
flowchart TB
    subgraph U["User picks a file"]
        A["OS file picker / intent"]
    end

    subgraph FE["Frontend (Svelte 5) — package:ui + package:platform"]
        P["@viewit/platform
           pickFile / open / mediaUrl / filePolicy
           (the ONLY seam; no raw Tauri/Capacitor in UI)"]
        V["@viewit/ui — shared viewers
           Text, Image, Pdf, Docx, Xlsx, Pptx, Archive, Media…"]
    end

    subgraph BE["Backend"]
        direction TB
        B1["Tauri Rust commands (Desktop & Android)"]
        B2["Stream server (127.0.0.1) — Range + CORS"]
        C["core: open() → Dish, Document resolver"]
        FMT["crates/fmt-* parsers
           (native thread / plugin .so)"]
        MAT["materialize → app-private cache
           (scoped-storage workaround)"]
    end

    subgraph W["Web target (apps/web)"]
        W1["WASM — fmt-*-universal + wasm_entry"]
    end

    subgraph IP["Plugin distribution"]
        CAT["plugins/catalog.signed.json (Ed25519)"]
        PL["plugins/* source + .zip"]
    end

    A --> P
    P --> B1
    P --> W1
    B1 --> C
    C --> FMT
    C --> MAT
    MAT --> B2
    FMT --> B2
    B2 -. bytes over IPC / stream .-> V
    V -. render .-> P
    BE -. verifies signature .-> CAT
    CAT -. signed manifest .-> PL
    PL -. plugin lookup .-> FMT
```

## Target → transport

| Target | Command host | Byte transport to viewer |
|---|---|---|
| Desktop (`apps/desktop`) | Tauri IPC (raw) | `stream_url` over localhost HTTP, Range-supported |
| Android (`apps/mobile`) | Tauri IPC (base64) | bytes materialized to app-private cache, read over IPC or `stream_url` (CORS-enabled) |
| Web (`apps/web`) | WASM (`fmt-*-universal`) | in-process memory; no network |

## Why the Android path uses the app cache

Android scopes `/sdcard/Download` access. When `MANAGE_EXTERNAL_STORAGE` is
revoked, the raw-URI stream read returns `EACCES`, and the stream server
previously returned a **non-CORS** 500 — which the WebView reports as an opaque
`TypeError: Failed to fetch` (`<video>/<audio>` → `code 4`). Two consistent
fixes:

1. Materialize shared files into the app-private cache at open time and serve
   bytes from there (`std::fs`, Range-supported) — permission-independent.
2. All stream-server responses (success *and* error) carry
   `Access-Control-Allow-Origin`, so failures surface as readable HTTP status.

See ADR `docs/adr/0010-webview-pdf-and-asset-streaming.md`.

## Viewing this diagram

The Mermaid source above renders on GitHub and in most Markdown viewers. For a
standalone SVG: `npx -y @mermaid-js/mermaid-cli -i docs/architecture/architecture.mmd`.
