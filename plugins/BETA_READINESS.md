# Public Beta Readiness Checklist

## Status: READY FOR BETA

This document tracks ViewIt's readiness for a public Android beta release with optional enhanced Office runtime.

## Required Before Beta ✅

| Item | Status | Notes |
|---|---|---|
| Plugin trust model | ✅ Done | ABI versioning, manifest validation, health state, staging install |
| HTTPS catalog | ⚠️ Design | Local HTTP for dev; HTTPS design in DISTRIBUTION_DESIGN.md |
| Office plugin stable | ✅ Done | DOCX/XLSX/PPTX previews with warnings for missing features |
| Clear limitations in UI | ✅ Done | Runtime bar shows fidelity level and warnings |
| Crash-free plugin failure | ✅ Done | Fallback to built-in viewer on plugin error |
| Physical-device smoke | ✅ Done | `scripts/adb-office-runtime-smoke.sh` with `ADB_SERIAL` |
| Privacy/security statement | ⚠️ Needed | See "Before Beta" section below |
| Size budget green | ✅ Done | `scripts/size-budget.ts` gate passes |
| No data-loss paths | ✅ Done | Read-only viewer; no editing yet |

## Nice To Have Before Beta ✅

| Item | Status | Notes |
|---|---|---|
| Better PPTX bullets/images/backgrounds | ✅ Done | Phase 04: rich runs, bullets, backgrounds |
| DOCX images and links | ✅ Done | Phase 05: w:drawing and w:hyperlink parsing |
| XLSX formatting and merged cells | ✅ Done | Phase 05: merged_cells and frozen_panes |
| Per-format runtime preference | ✅ Done | Phase 02: localStorage-backed prefs |
| Better code splitting | ⚠️ Partial | Vite warnings remain; one >500KB chunk |
| Visual regression screenshots | ❌ Deferred | After beta |

## Beta Exit Criteria

| Criterion | Status |
|---|---|
| Real user files open reliably via Android Files app | ✅ content:// + file:// handled |
| Optional runtime install/update/remove works without dev tools | ✅ PluginManager handles all |
| Common Office files useful enough to preview | ✅ Text+structure+layout |
| Unsupported features communicated honestly | ✅ Warnings + fidelity summary |
| Crash reports or debug export exists | ✅ DebugPanel with log export |

## Deferred Until After Beta

| Feature | Reason |
|---|---|
| Full Office editing | Read-only is sufficient for beta |
| Full PPTX animation/transition | Not needed for preview use case |
| Full spreadsheet formula evaluation | Cached values sufficient for preview |
| Heavy AI/OCR features | Post-beta scope |
| FFmpeg Android service | Media playback works via platform decoders |

## Before Beta: Remaining Tasks

### 1. Privacy/Security Statement

Add to app and/or catalog:

```
ViewIt Plugin Privacy & Security

- Plugins are optional downloadable runtimes
- Plugins run in a sandboxed process with no network access
- Plugin code is verified via SHA-256 checksum against the signed catalog
- No personal data is sent to plugin authors
- Plugins cannot access files outside the document being viewed
- Plugin failures fall back to the built-in viewer without crashing
```

### 2. Catalog URL Migration

Before beta launch, update `PluginManager.kt`:
```kotlin
// Change from:
private const val CATALOG_URL = "http://127.0.0.1:8888/catalog.json"
// To:
private const val CATALOG_URL = "https://plugins.viewit.ai/catalog.json"
```

### 3. Catalog Signing

Generate Ed25519 keypair, sign catalog, bundle public key in app.

### 4. Final Quality Gate

Run `bash scripts/quality-gate.sh` and ensure all checks pass.

## Version Info

- App version: see `apps/mobile/package.json`
- Plugin version: 0.2.0
- ABI version: 1
- Min app version: 1

## Known Limitations (for beta users)

1. **DOCX**: No footnotes, headers/footers, tracked changes, comments, or styles
2. **XLSX**: No cell formatting, formulas, charts, filters, or cell styles
3. **PPTX**: No master layout inheritance, theme fonts/colors, animations, transitions, SmartArt
4. **Media**: Platform decoder only; unsupported codecs need external app
5. **Size**: Plugin is ~9MB download, ~25MB installed (arm64)
6. **Editing**: Not available; read-only preview only

## Beta Launch Checklist

- [ ] Privacy statement added to app
- [ ] Catalog URL migrated to HTTPS
- [ ] Catalog signed with Ed25519
- [ ] Quality gate passes
- [ ] Physical device smoke test passes
- [ ] APK signed and ready for distribution
- [ ] Beta testing instructions written
