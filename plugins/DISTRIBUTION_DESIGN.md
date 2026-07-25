# Distribution, Catalog, Updates, and Rollback Design

## Overview

This document describes the design for safely shipping optional ViewIt runtimes (plugins) to users with update, rollback, and channel controls.

## Catalog Schema

### Stable Catalog Schema (v1)

```json
{
  "catalog_version": 1,
  "updated_at": "2024-01-15T00:00:00Z",
  "plugins": [
    {
      "id": "office-ooxml",
      "name": "Enhanced Office OOXML",
      "description": "Optional enhanced DOCX/XLSX/PPTX runtime",
      "version": "0.2.0",
      "abiVersion": 1,
      "entryClass": "ai.viewit.plugins.office_ooxml.Plugin",
      "capabilities": ["view", "inspect", "edit-planned"],
      "runtime": "android-dex-rust-sidecar",
      "supportedFormats": ["docx", "xlsx", "pptx", "docm", "xlsm", "pptm"],
      "minAppVersion": 1,
      "channels": {
        "stable": {
          "arm64-v8a": {
            "downloadUrl": "https://plugins.viewit.ai/office-ooxml/0.2.0/arm64-v8a.zip",
            "sizeBytes": 9055350,
            "installedSizeBytes": 25195042,
            "checksum": "sha256:..."
          },
          "x86_64": {
            "downloadUrl": "https://plugins.viewit.ai/office-ooxml/0.2.0/x86_64.zip",
            "sizeBytes": 9825290,
            "installedSizeBytes": 28762186,
            "checksum": "sha256:..."
          }
        },
        "beta": { ... },
        "dev": { ... }
      }
    }
  ]
}
```

## Release Channels

| Channel | Audience | Auto-update | Notes |
|---|---|---|---|
| `stable` | All users | Yes | Default channel; thoroughly tested |
| `beta` | Opt-in testers | Yes | Pre-release; may have bugs |
| `dev` | Developers | No | Nightly builds; unstable |

Channel selection stored in app preferences. Default: `stable`.

## Plugin Signing

### Approach: Signed Catalog + Checksum Verification

Rather than signing each plugin ZIP (which requires key distribution), we sign the catalog itself:

1. Catalog is signed with Ed25519 private key
2. App bundles the public key
3. App verifies catalog signature before trusting any plugin
4. Individual plugin integrity verified via SHA-256 checksum

```kotlin
data class SignedCatalog(
    val catalog: Catalog,
    val signature: String,  // base64 Ed25519 signature of catalog JSON
    val publicKey: String   // base64 public key (pinned in app)
)
```

## Update Flow

1. App starts → fetches catalog from `https://plugins.viewit.ai/catalog.json`
2. Verifies catalog signature
3. For each installed plugin:
   - Compare installed version with catalog version
   - If catalog > installed: mark as `UPDATE_AVAILABLE`
   - If catalog < installed and downgrade not allowed: ignore
4. User sees update badge in Plugin Store
5. User taps update → download → verify checksum → install → verify health

## Rollback Policy

### Automatic Rollback on Failed Update

1. Before installing new version, backup current version to `plugins/<id>.backup/`
2. Install new version to `plugins/<id>/`
3. If health check fails:
   - Remove failed installation
   - Restore from backup
   - Mark plugin as `FAILED` with error
   - Notify user

### Manual Rollback

User can manually rollback from Plugin Store:
- Settings → Plugins → [Plugin] → Rollback to v0.1.0

## Minimum App Version Enforcement

```kotlin
if (manifest.minAppVersion > BuildConfig.APP_VERSION_CODE) {
    throw PluginException("Plugin requires ViewIt v${manifest.minAppVersion} or higher")
}
```

App version code is incremented with each release. Plugin authors specify minimum compatible version.

## ABI-Specific Packages

Each plugin provides ABI-specific builds:
- `arm64-v8a` (primary Android target)
- `x86_64` (emulator, ChromeOS)
- `armeabi-v7a` (legacy, optional)

Catalog filters by device ABI at fetch time. Wrong ABI plugins are rejected at install.

## HTTPS Catalog Deployment

### Production

- Hosted on Cloudflare Pages or S3 + CloudFront
- URL: `https://plugins.viewit.ai/catalog.json`
- Catalog signed and checksum-verified

### Development

- Local HTTP server: `http://127.0.0.1:8888/catalog.json`
- `adb reverse tcp:8888 tcp:8888` for device testing
- **Never** shipped in production builds

### Catalog URL Configuration

```kotlin
object PluginConfig {
    const val CATALOG_URL = BuildConfig.PLUGIN_CATALOG_URL
    // Debug: "http://127.0.0.1:8888/catalog.json"
    // Release: "https://plugins.viewit.ai/catalog.json"
}
```

## Update Available UI

Plugin Store shows:
- Green badge: "Update available" with version number
- Tapping opens update dialog with changelog
- User confirms → background download → install

## Remove/Rollback UI

Plugin Store shows for each installed plugin:
- Uninstall button
- Rollback button (if backup exists)
- Current version + health status

## Release Checklist for Plugin ZIPs

- [ ] Bump version in `plugin.json`
- [ ] Run `cargo test` in plugin crate
- [ ] Run `bash build.sh` to generate ZIPs
- [ ] Verify ZIP checksums
- [ ] Update catalog with new version/checksums
- [ ] Sign catalog
- [ ] Upload ZIPs to CDN
- [ ] Upload signed catalog
- [ ] Verify catalog signature
- [ ] Test install on physical device
- [ ] Test update from previous version
- [ ] Test rollback

## Risks

- Signing key compromise → attacker can push malicious catalog
- Catalog schema change → older apps can't parse new catalog
- Local dev URLs leak to production → security issue
- Rollback backup consumes 2x storage temporarily

## Status

- Catalog schema v1: designed
- Release channels: designed
- Plugin signing: designed
- Update flow: designed
- Rollback policy: designed
- ABI packages: implemented (catalog filters by ABI)
- HTTPS deployment: not implemented (using local HTTP for dev)
- Update UI: not implemented
- Rollback UI: not implemented

## Next Steps

1. Implement catalog signing with Ed25519
2. Add update check on app start
3. Add update available badge in Plugin Store
4. Implement automatic rollback on failed update
5. Set up HTTPS catalog hosting
