# Universal Archive/Compression Plugin for ViewIt — Research & Architecture Document

## Executive Summary

ViewIt currently has a `fmt-archive` crate (Rust) that handles:
- **ZIP** (via `zip` crate)
- **TAR** (via `tar` crate)
- **TAR.GZ** (via `tar` + `flate2`)
- **7z** (optional, via `sevenz-rust` 0.6.1)

Missing formats: **.rar, .gz (plain), .bz2, .xz, .zst, .lz4, .lzma** + nested archives (.tar.bz2, .tar.xz, .tar.zst, .tar.lz4, .tar.lzma)

---

## 1. Best Rust Libraries for Each Format (WASM Compatible)

| Format | Library | Version | WASM | Streaming | Notes |
|--------|---------|---------|------|-----------|-------|
| **ZIP** | `zip` | 8.6.0 | ✅ | ✅ | Already used; supports deflate, bzip2, zstd, lzma |
| **TAR** | `tar` | 0.4.46 | ✅ | ✅ | Already used; no compression built-in |
| **GZIP** | `flate2` | 1.1.9 | ✅ | ✅ | Already used; `miniz_oxide` backend = pure Rust/WASM |
| **BZIP2** | `bzip2` | 0.6.1 | ✅ | ✅ | Pure Rust (`bzip2` feature) or C lib |
| **XZ/LZMA** | `xz2` | 0.1.7 | ⚠️ | ✅ | C library (liblzma); WASM needs `liblzma-sys` |
| **LZMA** | `lzma` | 0.2.2 | ✅ | ✅ | Pure Rust LZMA decoder |
| **ZSTD** | `zstd` | 0.13.3 | ✅ | ✅ | `zstd-safe` = pure Rust; `zstd` = C lib |
| **LZ4** | `lz4` | 1.28.1 | ✅ | ✅ | `lz4_flex` = pure Rust alternative |
| **7Z** | `sevenz-rust` | 0.6.1 | ⚠️ | ❌ | Requires full extraction for listing; optional feature |
| **RAR** | `unrar` | 0.5.8 | ❌ | ✅ | C++ library (unrar.dll); NO WASM support |

### WASM Compatibility Notes

| Library | Pure Rust? | WASM Status |
|---------|------------|-------------|
| `zip` (with `deflate`, `bzip2`, `zstd`, `lzma` features) | Yes (miniz_oxide, bzip2, zstd-safe, lzma) | ✅ Full |
| `tar` | Yes | ✅ Full |
| `flate2` (with `miniz_oxide`) | Yes | ✅ Full |
| `bzip2` (with `bzip2` feature) | Yes | ✅ Full |
| `xz2` | No (liblzma C) | ⚠️ Needs `liblzma-sys` WASM build |
| `lzma` | Yes | ✅ Full |
| `zstd` (with `zstd-safe`) | Yes | ✅ Full |
| `lz4_flex` | Yes | ✅ Full |
| `sevenz-rust` | Yes (mostly) | ⚠️ Large, slow in WASM |
| `unrar` | No (C++) | ❌ Impossible in WASM |

---

## 2. Best JavaScript Libraries for Each Format

| Format | Library | Size (min+gzip) | Streaming | Notes |
|--------|---------|-----------------|-----------|-------|
| **ZIP** | `zip.js` / `@zip.js/zip.js` | ~45 KB | ✅ | Web Workers, streaming, WASM codecs |
| **TAR** | `tar-stream` | ~5 KB | ✅ | Pure streaming |
| **GZIP** | `pako` / `fflate` | ~8 KB / ~3 KB | ✅ | `fflate` is smaller, faster |
| **BZIP2** | `unbzip2` / `bzip2.js` | ~25 KB | ⚠️ | Limited streaming |
| **XZ/LZMA** | `lzma-native` / `xz.js` | ~100 KB | ⚠️ | Large WASM binary |
| **ZSTD** | `zstd-js` / `@bokuweb/zstd-wasm` | ~200 KB | ✅ | Large WASM |
| **LZ4** | `lz4js` / `lz4-wasm` | ~20 KB | ✅ | |
| **7Z** | `7zip-min.js` / `wasm-7z` | ~500 KB | ❌ | Very large |
| **RAR** | `unrar.js` / `rar.js` | ~300 KB | ❌ | Proprietary, large |

### Recommended JS Stack (Minimal Bundle)
```json
{
  "dependencies": {
    "fflate": "^0.8.0",           // gzip, zlib, deflate, raw deflate (~3 KB)
    "tar-stream": "^3.0.0",       // tar streaming (~5 KB)
    "@zip.js/zip.js": "^2.7.0",   // ZIP with WASM codecs (~45 KB)
    "lzma-native": "^1.0.0",      // LZMA/XZ (~100 KB)
    "unbzip2": "^1.0.0"           // BZIP2 (~25 KB)
  }
}
```
**Estimated JS bundle: ~180 KB gzipped** (vs 10-15 MB for Rust WASM)

---

## 3. Rust vs JavaScript Approach — Pros/Cons

### Rust (WASM) Approach

| Pros | Cons |
|------|------|
| ✅ Native performance (near-C speed) | ❌ **Bundle size: 5-15 MB** (even with `opt-level="z"`) |
| ✅ Single codebase with core `fmt-archive` | ❌ WASM compilation complexity |
| ✅ Streaming via `Read` trait | ❌ No RAR support (C++ unrar) |
| ✅ Memory safety | ❌ Large WASM for 7z, zstd, lzma |
| ✅ Reuses existing `ArchiveEntry` types | ❌ Debugging harder |
| ✅ Can compile to Android JNI/DEX (current plugin model) | ❌ Slow iteration (rebuild) |

**Estimated WASM Sizes (release, opt-level=z, LTO):**
| Feature Set | Est. Size |
|-------------|-----------|
| ZIP + TAR + GZIP only | ~1.2 MB |
| + BZIP2 + LZMA (pure Rust) | ~2.5 MB |
| + ZSTD (zstd-safe) | ~3.5 MB |
| + LZ4 (lz4_flex) | ~4.0 MB |
| + 7z (sevenz-rust) | ~6-8 MB |
| **All formats (no RAR)** | **~8-12 MB** |

### JavaScript Approach

| Pros | Cons |
|------|------|
| ✅ **Tiny bundle: ~180 KB gzipped** | ❌ Slower decompression (JS/WASM overhead) |
| ✅ Instant iteration (no rebuild) | ❌ Memory pressure on large archives |
| ✅ Native Web Workers for off-main-thread | ❌ Limited streaming for some formats |
| ✅ Easy RAR via `unrar.js` (WASM) | ❌ Duplicate logic with Rust core |
| ✅ Works in web/desktop/mobile WebView | ❌ Two codebases to maintain |

### Hybrid Approach (Recommended)

**Rust core for ZIP/TAR/GZIP** (already in `fmt-archive`, ~1.2 MB WASM)
**JS for exotic formats** (BZIP2, XZ, ZSTD, LZ4, 7Z, RAR)

---

## 4. Recommended Architecture

### Option A: Single Plugin (Rust WASM + JS fallback)
```
viewit-plugin-archive/
├── Cargo.toml           # Rust → WASM (ZIP, TAR, GZIP, BZIP2, LZMA, ZSTD, LZ4)
├── src/
│   ├── lib.rs           # Main entry: parse() → Document::Archive
│   ├── zip.rs           # ZIP listing (streaming)
│   ├── tar.rs           # TAR listing (streaming)
│   ├── gzip.rs          # GZIP detection + single-file
│   ├── bzip2.rs         # BZIP2 streaming
│   ├── xz.rs            # XZ/LZMA streaming
│   ├── zstd.rs          # ZSTD streaming
│   ├── lz4.rs           # LZ4 streaming
│   ├── sevenz.rs        # 7z listing (optional feature)
│   ├── nested.rs        # tar.gz, tar.xz, tar.zst, tar.bz2, tar.lz4, tar.lzma
│   └── extract.rs       # Extraction API (stubbed for read-only)
├── js/
│   ├── archive.js       # JS entry: detect format → call Rust or JS lib
│   ├── unrar.js         # RAR via unrar.js (WASM)
│   ├── sevenz.js        # 7z via wasm-7z (fallback)
│   └── index.ts         # Plugin manifest + UI integration
├── plugin.json          # Manifest
└── build.rs             # wasm-pack build config
```

### Option B: Multiple Plugins (Modular)
| Plugin ID | Formats | Runtime | Size |
|-----------|---------|---------|------|
| `archive-core` | zip, tar, gz | Rust WASM | ~1.2 MB |
| `archive-bzip2` | bz2, tar.bz2 | Rust WASM | +300 KB |
| `archive-xz` | xz, lzma, tar.xz, tar.lzma | Rust WASM | +500 KB |
| `archive-zstd` | zst, tar.zst | Rust WASM | +800 KB |
| `archive-lz4` | lz4, tar.lz4 | Rust WASM | +400 KB |
| `archive-7z` | 7z | Rust WASM | +2 MB |
| `archive-rar` | rar | JS (unrar.js) | ~300 KB JS |

**Recommendation: Option A (Single Plugin)** — simpler distribution, user preference "I do not care about size... but preferred under 10-15 MB"

---

## 5. Integration with ViewIt Plugin System

### Plugin Manifest (`plugin.json`)
```json
{
  "id": "archive-universal",
  "name": "Universal Archive Viewer",
  "version": "1.0.0",
  "description": "List, preview, and extract ZIP, 7Z, RAR, TAR, GZ, BZ2, XZ, ZSTD, LZ4, LZMA archives",
  "minAppVersion": 1,
  "abiVersion": 1,
  "entryClass": "ai.viewit.plugins.archive_universal.Plugin",
  "capabilities": ["view", "inspect", "extract-planned"],
  "runtime": "wasm",
  "supportedFormats": [
    "zip", "7z", "rar", "tar", "gz", "tgz",
    "bz2", "tbz2", "tbz", "xz", "txz", "tlz",
    "zst", "tzst", "lz4", "tlz4", "lzma"
  ],
  "downloadUrl": "https://github.com/viewit/plugins/releases/download/archive-universal-1.0.0.zip",
  "sizeBytes": 12582912,
  "installedSizeBytes": 14680064,
  "checksum": "sha256:..."
}
```

### Plugin Bridge Integration (TypeScript)
```typescript
// packages/ui/src/plugins/archiveUniversal.ts
import { renderDocumentWithPlugin } from './pluginBridge';

export async function openArchiveWithPlugin(uri: string, ext: string) {
  const plugin = await findPlugin('archive-universal');
  if (!plugin) throw new Error('Archive plugin not installed');
  
  const doc = await renderDocumentWithPlugin(plugin, uri, ext);
  return doc as Document; // Document::Archive variant
}
```

### Rust WASM Entry Point
```rust
// plugins/archive-universal/src/lib.rs
use wasm_bindgen::prelude::*;
use viewit_core_types::{Document, Format, ArchiveEntry, Error};

#[wasm_bindgen]
pub fn parse(bytes: &[u8], format: &str, name: &str) -> Result<JsValue, JsValue> {
    let fmt = parse_format(format);
    let entries = list_archive(bytes, fmt)?;
    let doc = Document::Archive { entries, format: fmt, byte_len: bytes.len() };
    Ok(serde_wasm_bindgen::to_value(&doc)?)
}

#[wasm_bindgen]
pub fn extract_entry(bytes: &[u8], format: &str, entry_name: &str) -> Result<Vec<u8>, JsValue> {
    // Future: extraction support
    Err("Not yet implemented".into())
}
```

---

## 6. Handling Nested Archives (tar.gz, tar.xz, etc.)

### Detection Strategy
```rust
fn detect_nested(bytes: &[u8]) -> Option<(Compression, Format)> {
    // 1. Check magic bytes for outer compression
    // 2. Decompress first 512 bytes
    // 3. Check for "ustar" magic (TAR)
    // 4. Return (Compression::Gzip, Format::ArchiveTar) etc.
}
```

### Supported Nested Combinations
| Extension | Outer | Inner | Detection |
|-----------|-------|-------|-----------|
| `.tar.gz` / `.tgz` | GZIP | TAR | Magic 0x1f8b → ustar |
| `.tar.bz2` / `.tbz2` | BZIP2 | TAR | Magic BZh → ustar |
| `.tar.xz` / `.txz` | XZ | TAR | Magic 0xfd7zXZ → ustar |
| `.tar.zst` / `.tzst` | ZSTD | TAR | Magic 0x28b52ffd → ustar |
| `.tar.lz4` / `.tlz4` | LZ4 | TAR | Magic 0x04224d18 → ustar |
| `.tar.lzma` / `.tlz` | LZMA | TAR | Magic 0x5d000080 → ustar |

### Implementation (Streaming)
```rust
fn list_nested_tar<R: Read>(reader: R, compression: Compression) -> Result<Vec<ArchiveEntry>> {
    let decompressed: Box<dyn Read> = match compression {
        Compression::Gzip => Box::new(flate2::read::GzDecoder::new(reader)),
        Compression::Bzip2 => Box::new(bzip2::read::BzDecoder::new(reader)),
        Compression::Xz => Box::new(xz2::read::XzDecoder::new(reader)),
        Compression::Zstd => Box::new(zstd::stream::Decoder::new(reader)?),
        Compression::Lz4 => Box::new(lz4_flex::frame::FrameDecoder::new(reader)),
        Compression::Lzma => Box::new(lzma::read::LzmaDecoder::new(reader)),
    };
    tar::Archive::new(decompressed).entries()?.filter_map(Result::ok).map(|e| {
        let header = e.header();
        ArchiveEntry {
            name: header.path()?.to_string_lossy().into_owned(),
            size: header.size()?,
            is_dir: header.entry_type().is_dir(),
            compressed_size: header.size()?, // Tar doesn't store per-file compressed size
        }
    }).collect()
}
```

---

## 7. Streaming / Chunked Processing for Large Archives

### Requirements
- **Don't load entire archive into memory** (current `fmt-archive` loads all bytes)
- **List entries without full extraction** (metadata only)
- **Extract single file on demand** (stream to disk/memory)

### Rust Streaming Architecture
```rust
// Trait for streaming archive readers
pub trait ArchiveReader: Send + Sync {
    fn list_entries(&mut self) -> Result<Vec<ArchiveEntry>, Error>;
    fn extract_entry(&mut self, name: &str, writer: &mut dyn Write) -> Result<u64, Error>;
    fn extract_all(&mut self, dir: &Path) -> Result<(), Error>;
}

// ZIP: zip::ZipArchive supports streaming via Cursor<Vec<u8>> but needs full central dir
// For true streaming: zip::ZipArchive::new with seekable reader (file/HTTP range)

// TAR: tar::Archive is naturally streaming (reads sequentially)

// Compressed TAR: Chain decoders (GzDecoder → tar::Archive) — fully streaming
```

### Memory-Efficient Listing (Current fmt-archive Issue)
```rust
// CURRENT: loads entire file into bytes: &[u8]
pub fn parse(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error>

// FUTURE: Accept Read + Seek for streaming
pub fn parse_stream<R: Read + Seek>(reader: &mut R, format: Format, name: &str) -> Result<Document, Error>

// For HTTP: implement Range requests via custom Read+Seek wrapper
pub struct HttpRangeReader { url: String, client: Client, pos: u64, len: u64 }
impl Read for HttpRangeReader { ... }
impl Seek for HttpRangeReader { ... }
```

### WASM Consideration
- WASM cannot access filesystem directly
- Use `web-sys::ReadableStream` for HTTP streaming
- For file:// URLs: use FileReader API in chunks

---

## 8. Preview Rendering of Contained Files

### Reuse Existing ViewIt Viewers
```typescript
// packages/ui/src/components/ArchiveViewer.svelte
<script>
  import { TextViewer } from './TextViewer.svelte';
  import { ImageViewer } from './ImageViewer.svelte';
  import { PdfViewer } from './PdfViewer.svelte';
  import { CodeViewer } from './CodeViewer.svelte';
  import { open } from '$lib/core/api'; // Rust core via WASM/Tauri
</script>

{#if selectedEntry}
  <div class="preview-pane">
    {#await previewPromise}
      <LoadingSpinner />
    {:then doc}
      {#if doc.kind === 'text'}
        <TextViewer content={doc.content} />
      {:else if doc.kind === 'image'}
        <ImageViewer src={doc.stream_url} />
      {:else if doc.kind === 'pdf'}
        <PdfViewer url={doc.stream_url} />
      {:else if doc.kind === 'code'}
        <CodeViewer content={doc.content} lang={detectLang(entry.name)} />
      {:else}
        <UnsupportedPreview format={doc.format} />
      {/if}
    {:catch err}
      <ErrorMessage {err} />
    {/await}
  </div>
{/if}
```

### Preview Flow
1. User clicks entry in archive list
2. Plugin extracts entry to memory (streaming, not full archive)
3. Dispatch to `core::open(bytes, ext, name)` → `Document`
4. Render appropriate viewer component (reuse existing)

### Entry Extraction API (Future)
```rust
#[wasm_bindgen]
pub fn extract_entry_to_bytes(archive_bytes: &[u8], format: &str, entry_path: &str) -> Result<Vec<u8>, JsValue> {
    // Stream single entry without full extraction
    let mut reader = ArchiveReader::new(archive_bytes, format)?;
    let mut buf = Vec::new();
    reader.extract_entry(entry_path, &mut buf)?;
    Ok(buf)
}
```

---

## 9. Format Support Matrix (Implementation Priority)

| Priority | Format | Rust Lib | JS Fallback | Streaming | Nested | Size Impact |
|----------|--------|----------|-------------|-----------|--------|-------------|
| 1 | ZIP | `zip` | `zip.js` | ✅ | - | +0 KB (existing) |
| 1 | TAR | `tar` | `tar-stream` | ✅ | - | +0 KB (existing) |
| 1 | GZIP | `flate2` | `fflate` | ✅ | tar.gz | +0 KB (existing) |
| 2 | BZIP2 | `bzip2` | `unbzip2` | ✅ | tar.bz2 | +150 KB |
| 2 | XZ/LZMA | `lzma` | `lzma-native` | ✅ | tar.xz | +300 KB |
| 2 | ZSTD | `zstd-safe` | `zstd-js` | ✅ | tar.zst | +500 KB |
| 2 | LZ4 | `lz4_flex` | `lz4js` | ✅ | tar.lz4 | +200 KB |
| 3 | 7Z | `sevenz-rust` | `wasm-7z` | ❌ | - | +2 MB |
| 3 | RAR | ❌ | `unrar.js` | ❌ | - | +300 KB JS |

---

## 10. Recommended Implementation Plan

### Phase 1: Core Streaming Refactor (1-2 weeks)
- [ ] Change `fmt-archive::parse()` to accept `Read + Seek`
- [ ] Add `HttpRangeReader` for HTTP streaming
- [ ] Implement streaming ZIP listing (read central dir only)
- [ ] Add `Compression` enum + `detect_nested()`

### Phase 2: Add Missing Formats in Rust (2-3 weeks)
- [ ] BZIP2 (pure Rust `bzip2` crate)
- [ ] LZMA/XZ (pure Rust `lzma` crate)
- [ ] ZSTD (pure Rust `zstd-safe`)
- [ ] LZ4 (pure Rust `lz4_flex`)
- [ ] Nested tar.* handlers

### Phase 3: WASM Plugin Build (1 week)
- [ ] `wasm-pack` configuration for `archive-universal`
- [ ] Optimize bundle size (`opt-level=z`, `lto=true`, `strip=true`)
- [ ] Test in WebView (Android, Desktop, Web)

### Phase 4: JS Fallback for 7Z/RAR (1 week)
- [ ] Integrate `unrar.js` for RAR
- [ ] Integrate `wasm-7z` or `7zip-min.js` for 7Z
- [ ] Format detection → route to Rust or JS

### Phase 5: UI Integration (1 week)
- [ ] Archive list virtualization (large archives)
- [ ] Preview pane (reuse existing viewers)
- [ ] Extraction UI (download selected / extract all)

### Phase 6: Polish (1 week)
- [ ] Password-protected archives (prompt UI)
- [ ] Progress indicators for large extractions
- [ ] Error handling + user-friendly messages
- [ ] Documentation + plugin catalog entry

---

## 11. Estimated Final Bundle Sizes

| Configuration | Rust WASM | JS Fallback | Total |
|---------------|-----------|-------------|-------|
| Minimal (zip, tar, gz) | 1.2 MB | 0 | **1.2 MB** |
| Standard (+bz2, xz, zst, lz4) | 4.5 MB | 0 | **4.5 MB** |
| Full (+7z) | 8-12 MB | 0 | **8-12 MB** |
| Full + RAR (JS) | 8-12 MB | 300 KB | **8.3-12.3 MB** |

**All under the 10-15 MB budget** ✅

---

## 12. Key Dependencies (Cargo.toml)

```toml
[package]
name = "viewit-fmt-archive"
version.workspace = true
edition.workspace = true

[features]
default = ["zip", "tar", "gzip"]
zip = ["dep:zip"]
tar = ["dep:tar"]
gzip = ["dep:flate2"]
bzip2 = ["dep:bzip2"]
lzma = ["dep:lzma"]
zstd = ["dep:zstd"]
lz4 = ["dep:lz4_flex"]
sevenz = ["dep:sevenz-rust"]
full = ["zip", "tar", "gzip", "bzip2", "lzma", "zstd", "lz4", "sevenz"]

[dependencies]
viewit-core-types = { path = "../../core-types" }
anyhow.workspace = true
thiserror.workspace = true
serde.workspace = true
bytes.workspace = true

# Core (always)
zip = { version = "=8.6.0", default-features = false, features = ["deflate", "bzip2", "zstd", "lzma"], optional = true }
tar = { version = "0.4", optional = true }
flate2 = { version = "1", features = ["miniz_oxide"], optional = true }

# Additional compression (pure Rust for WASM)
bzip2 = { version = "0.6", features = ["bzip2"], optional = true }
lzma = { version = "0.2", optional = true }
zstd = { version = "0.13", features = ["zstd-safe"], optional = true }
lz4_flex = { version = "0.11", optional = true }

# 7z (optional, large)
sevenz-rust = { version = "=0.6.1", optional = true }

# WASM bindings
wasm-bindgen = { version = "0.2", optional = true }
serde-wasm-bindgen = { version = "0.5", optional = true }
```

---

## 13. Conclusion & Recommendation

**Go with Rust WASM single plugin** using pure-Rust compression libraries:
- ✅ Fits 10-15 MB budget (4.5 MB for all non-7z formats)
- ✅ Single codebase, reuses `fmt-archive` types
- ✅ Streaming architecture for large archives
- ✅ Compiles to Android JNI/DEX (current plugin model)
- ❌ No RAR in Rust — add JS `unrar.js` fallback (~300 KB)
- ❌ 7z is large (+2-4 MB) — make it optional feature

**Start with Phase 1-2** to extend `fmt-archive` with streaming + missing formats, then package as WASM plugin.
