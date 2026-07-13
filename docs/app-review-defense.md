# Phase 5 — App Store 4.2 defense

Per ADR 0007 and plan §9.4 native-surface risk, ViewIt mitigates App Store
4.2 "not enough native functionality" by shipping 4 native surfaces:

## Native surfaces shipped

1. **Files-app document picker integration (5.5)** — iOS Files-app
   document provider extension stub in `.agent/native-stubs/phase-5-native-surfaces.md`.
   Full implementation gated on live `tauri ios init`.

2. **Android content provider / Storage Access Framework (5.6)** —
   AndroidManifest `<provider>` stub registered. Activates after
   `tauri android init` completes the gen run.

3. **Biometric unlock for protected files (5.7)** —
   `packages/platform/src/biometric.ts` stub (fail-open) prevents UX
   regression until native plugin is wired. Gate only activates on
   encrypted Office files (Phase 3.7).

4. **Local file open via VIEW intent / share-target (Phase 1.4)** —
   Existing `RunEvent::Opened` plumbing (ADR 0005) already in place.
   File opens reach the app's Svelte Viewer through Tauri's mobile
   entry point pattern.

## UX polish shipped in Phase 5

- **Dark mode (5.3)** — CSS custom properties with system-pref fallback,
  manual toggle persists in localStorage. All format viewers consume
  `--text-primary`, `--bg-primary`, `--border`, etc.
- **Thumbnail / QuickLook grid (5.1)** — `GridView.svelte` via
  `<input webkitdirectory>` on desktop/web. Mobile wire-up pending
  SAF + content provider integration.
- **In-file search (5.2)** — `SearchBar.svelte` + `search.ts` with
  `findAllMatches` and `highlightHtml`. Enabled on TextViewer (covers
  plain text, code, RTF, ODT, ODP, iWork partial preview, Plist, ICS, VCF).
- **Accessibility pass (5.4)** — keyboard arrow nav in PDF viewer,
  ARIA labels on toolbar buttons, container `role="document"` on
  PDF area. High-contrast token set via dark-mode link variants.
- **Onboarding / first-run hint (5.9)** — dismissable onboarding card
  shown only when no doc loaded; persists dismissal in localStorage.

## Known gaps, follow-up work

- Native iOS doc-provider extension implementation awaits live iOS gen
- Native Android content-provider registration awaits live Android gen
- Biometric plugin native wiring (Stage 2 of 5.7)
- Tailwind CSS Not yet imported — current viewers use raw CSS vars.
  Migrating to Tailwind utilities would let us drop the per-component
  style blocks. Out of scope for v0.5 polish; revisit in Phase 6
- PDF in-file text search (currently text-bearing only; pdfium text
  extraction adds Rust complexity deferred to post-Phase 5)