# Public Beta Readiness

## Status: Blocked By Architecture Renewal

The previous checklist described an older Office-only plugin architecture and incorrectly claimed downloaded plugins ran in a sandboxed process. It is retained in Git history and is not current guidance.

Current downloadable Android extensions are trusted in-process code. Catalog signatures and artifact checksums verify metadata and bytes; they do not isolate execution.

Public beta readiness requires the applicable gates in:

- [`../docs/ANDROID-RELEASE-CHECKLIST.md`](../docs/ANDROID-RELEASE-CHECKLIST.md)
- [`../docs/PLUGIN-ARCHITECTURE.md`](../docs/PLUGIN-ARCHITECTURE.md)
- [`../docs/PLUGIN-DISTRIBUTION.md`](../docs/PLUGIN-DISTRIBUTION.md)

At minimum, beta review must cover canonical contracts, non-destructive updates, provider lifecycle/health, accurate trust disclosure, device validation, and an explicitly approved distribution profile.

An optimized `device-test` APK or AAB is not a public beta release.
