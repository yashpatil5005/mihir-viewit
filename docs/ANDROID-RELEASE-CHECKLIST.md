# Android Store Release Checklist

## Artifact
- [ ] `bash scripts/android-release.sh` passes (APK ≤ 15 MB).
- [ ] `bash scripts/verify-android-16kb.sh dist/viewit-android-arm64-release.apk` passes.
- [ ] Record `sha256sum dist/viewit-android-arm64-release.apk` in release notes.
- [ ] Build twice from a clean tree and compare APK hashes (or explain signing/timestamp drift).

## Version/signing
- [ ] Set `apps/mobile/src-tauri/tauri.conf.json` version and Android versionCode/versionName.
- [ ] Use the production Play App Signing upload key (never commit the keystore/password).
- [ ] Back up the upload key securely; CI secrets provide credentials.

## Privacy/security copy
- [ ] Plugin downloads are optional, signed (Ed25519 catalog) and checksum-verified.
- [ ] No document content is uploaded; format parsing/rendering is local/offline after plugin install.
- [ ] Disclose network use: plugin catalog/downloads and optional custom catalog URLs.
- [ ] Confirm plugin storage scope and capabilities are visible in Plugin Store.
- [ ] Link `SECURITY.md` and `docs/app-review-defense.md` in store-review material.

## QA
- [ ] `bash scripts/adb-release-qa.sh` on a physical arm64 device.
- [ ] Fresh install: zero baked plugins.
- [ ] Default signed catalog loads without manual URL.
- [ ] Network install: office/compression/font/player/editor/iwork plugins.
- [ ] Core matrix: text/image/pdf/media/docx/xlsx/pptx/archive/font/iWork.
- [ ] Native update: staged + restart-to-apply.
- [ ] Warm remove/reinstall: no native `.so` namespace failure.

## Distribution
- [ ] `CLOUDFLARE_PAGES_PROJECT=viewit-plugin-catalog-temp bash scripts/publish-plugins.sh`.
- [ ] `bash scripts/check-catalog.sh https://omnia.mihirpatil.co` → ALL PLUGINS OK.
- [ ] Tag + release notes include app/APK hash and catalog signature date.
