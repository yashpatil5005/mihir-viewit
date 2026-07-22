# ViewIt Streaming Architecture Plan

## Problem
Current flow reads ENTIRE file into memory before ANY processing:
1. `open_uri(uri)` → `read_uri_bytes()` → FULL FILE READ (Vec<u8>)
2. `open(&bytes, &ext, &name)` → Document
3. For PDF/Media: `materialize_uri_to_cache()` → SECOND full read + write
4. Frontend: `readMaterializedBytes()` → THIRD read → base64 decode

**Result**: 3x full file reads, 33% base64 overhead, JSON serialization, OOM on mobile.

## Solution: "Sniff First, Stream Always"

### New Flow
```
Frontend ──open_uri(uri)──▶ Rust ──read 256KB──▶ Core (sniff format)
     ▲                         │
     │                         ▼
     │              Document { stream_url, ... }
     │                         │
     └────── fetch(stream_url) ◀── Stream Server ──stream from content://──▶ Android SAF
```

### Key Changes

1. **`open_uri` reads only 256KB** for format detection
2. **Returns `stream_url`** (`http://127.0.0.1:PORT/stream/{id}`) for ALL formats
3. **Stream server reads from `content://` on-demand** — zero full-file buffering
4. **Frontend fetches via HTTP** — native WebView decoding, Range request support
5. **No materialization** for PDF/Media/Images (keep only for PPTX client-side viewer)
6. **No base64** — binary streaming via HTTP
7. **No JSON text** — large text fetched as raw HTTP response

### Document Changes

Add `stream_url: Option<String>` to:
- `Text` — large files fetched via HTTP
- `Image` — `<img src={stream_url}>`
- `Media` — `<video src={stream_url}>`
- `Pdf` — pdf.js fetches via stream_url
- `Pptx` — client-side viewer fetches via stream_url

### Files to Modify

| File | Change |
|------|--------|
| `apps/mobile/src-tauri/Cargo.toml` | Add `tiny_http` |
| `apps/mobile/src-tauri/src/stream_server.rs` | New: unified HTTP server |
| `apps/mobile/src-tauri/src/uri_util.rs` | Read 256KB only, register stream |
| `apps/mobile/src-tauri/src/materialize.rs` | Remove PDF/Media/Image materialization |
| `apps/mobile/src-tauri/src/lib.rs` | Add `stream_url` to commands, remove base64 |
| `crates/core-types/src/lib.rs` | Add `stream_url` to Document variants |
| `packages/ui/src/Viewer.svelte` | Handle stream_url |
| `packages/ui/src/MediaViewer.svelte` | Use stream_url directly |
| `packages/ui/src/PdfViewer.svelte` | Fetch via stream_url |
| `packages/ui/src/ImageViewer.svelte` | Use stream_url |
| `packages/platform/src/index.ts` | Add stream URL helpers |

## Implementation Phases

### Phase 1: Core Streaming (CRITICAL)
- [x] Research tiny_http + Tauri patterns
- [ ] Add tiny_http to Cargo.toml
- [ ] Create stream_server.rs with content:// streaming
- [ ] Add `register_stream_uri` command
- [ ] Modify `open_uri` to return stream_url for all formats

### Phase 2: Document Updates (CRITICAL)
- [ ] Add stream_url to Document variants
- [ ] Update uri_util.rs — 256KB sniff only
- [ ] Remove materialization for PDF/Media/Image

### Phase 3: Frontend Updates (CRITICAL)
- [ ] Update Viewer.svelte
- [ ] Update MediaViewer.svelte
- [ ] Update PdfViewer.svelte
- [ ] Update ImageViewer.svelte
- [ ] Update platform.ts

### Phase 4: Cleanup (HIGH)
- [ ] Remove base64 commands
- [ ] Remove read_materialized_bytes_b64
- [ ] Remove ensure_cached full-file read
- [ ] Update stream_protocol.rs

## Benchmark Plan

Before/after comparison on Samsung SM-E346B:
1. **10MB PDF** — open time, memory usage
2. **50MB video** — open time, seek latency, memory usage
3. **5MB image** — open time, render time
4. **1MB text** — open time, scroll performance
5. **100MB PDF** — open time (should work now, failed before)

Metrics:
- Time to first byte (TTFB)
- Time to full load
- Peak memory (RSS)
- Seek latency (video/PDF)
