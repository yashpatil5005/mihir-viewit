# ADR 0011: Generated Android project (`gen/android`) is not tracked

## Status

Accepted.

## Context

- Tauri v2 generates `apps/*/src-tauri/gen/android` (Gradle project: `MainActivity.kt`, `PluginManager.kt`, `build.gradle.kts`, …) **from** `apps/mobile/src-tauri/android/` + `tauri.conf.json` at build time.
- Some generated files were **committed before** `.gitignore` gained the `**/src-tauri/gen/` rule, so a redundant, drift-prone **mirror** of the authoritative source is still tracked.
- A bare `src-tauri/gen/` in `.gitignore` is anchored to the repo root and would **not** match `apps/*/src-tauri/gen/`; the rule therefore requires the `**/` prefix (see comment in [.gitignore](../.gitignore)).
- Tracking generated build output creates ambiguous policy: which tree is the source of truth when `gen/android` and `android/` disagree?

## Decision

1. **`gen/android` is NOT tracked.** `.gitignore` ignores `**/src-tauri/gen/` for all apps.
2. **Source of truth is `apps/mobile/src-tauri/android/`** — generated files under `gen/` are never treated as authoritative.
3. On merging any Android **source** change, contributors run `tauri android build` (via `bash scripts/android-release.sh`) so Tauri **regenerates** `gen/android`.
4. Any **already-tracked** gen mirrors are progressively **untracked** (`git rm --cached`) as part of cleanup.
5. **Never hand-edit** files under `gen/`; edits belong in `src-tauri/android/` or `tauri.conf.json`.

## Consequences

- **Positive:** a single source of truth; no accidental commits of generated artifacts; diffs are confined to product source.
- **Negative:** the `gen/` dir is transient and absent on a fresh checkout; regenerating it requires running the build; contributors must not edit it directly (an "edited gen file" is silently lost on the next regenerate).
- **Migration:** run `git rm -r --cached apps/mobile/src-tauri/gen/android` (dry-run with `--dry-run` first), relying on the existing `.gitignore` to keep them untracked, then rebuild to confirm regeneration is reproducible.
