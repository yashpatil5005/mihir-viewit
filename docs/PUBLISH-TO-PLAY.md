# Google Play Publication Reference

ViewIt is not currently production-ready. This document records the future account-bound Play Console procedure; it is not an instruction to publish the current device-test artifact.

The standard `scripts/android-release.sh` output uses the `device-test` profile unless `VIEWIT_APP_PROFILE=production` is explicitly set. Gradle release-mode optimization and an AAB do not establish production readiness.

## Blocking Gate

Complete [`ANDROID-RELEASE-CHECKLIST.md`](ANDROID-RELEASE-CHECKLIST.md) and obtain explicit user approval before performing any remote upload, release creation, catalog publication, or Play rollout.

## Future Production Build

```bash
VIEWIT_APP_PROFILE=production \
VIEWIT_PRODUCTION_SIGNING=1 \
ANDROID_SIGNING_CERT_SHA256=<approved-upload-certificate-sha256> \
ANDROID_SIGNING_KEYSTORE=/secure/path/viewit-upload.jks \
ANDROID_SIGNING_STORE_PASS=... \
ANDROID_SIGNING_KEY_PASS=... \
ANDROID_SIGNING_KEY_ALIAS=... \
bash scripts/android-release.sh
```

The script must fail closed if production signing is absent. Verify resulting APK/AAB signatures and hashes according to the checklist.

## Play Account Steps

After technical and user approval:

1. Create or use the intended Google Play developer account.
2. Create the ViewIt app entry with package `ai.viewit.app`.
3. Enroll in Play App Signing and register the approved upload certificate.
4. Complete store listing, privacy, data safety, content rating, and target audience.
5. Upload the approved production-profile AAB to the intended test track first.
6. Review automated/device reports and staged rollout behavior.
7. Promote to production only through a separate explicit decision.

## Key Custody

- Never commit keystores or passwords.
- Back up the upload key and credentials in a trusted vault.
- Record certificate fingerprints from the actual approved key at release time; do not rely on stale fingerprints in documentation.
- Document key rotation and compromise response before first production publication.

## Plugin Catalog

Plugin/catalog publication is a separate remote operation. Verify canonical metadata, signatures, artifact checksums, compatibility, update/rollback behavior, and catalog health before app rollout.
