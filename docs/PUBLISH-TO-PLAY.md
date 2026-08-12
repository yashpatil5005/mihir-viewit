# Publish ViewIt to Google Play

This doc is the single source of truth for the **one manual, account-bound step**
in shipping ViewIt to Google Play. Everything machine-verifiable is already done
by the release tooling; the remaining steps require your Google identity and card
and cannot be automated.

## What is ready (automated)

| Artifact | Where | How it's produced |
|---|---|---|
| Upload keystore `.jks` | `~/.android/viewit-upload.jks` (outside repo) | `keytool`, RSA 3072, alias `viewit-upload`, 30 yr |
| Upload credentials | `~/.android/viewit-upload.env` (mode `0600`) | store/key pass (random, non-secret-committed) |
| Upload cert SHA-256 | `85:9C:E4:90:B9:75:22:A8:40:60:14:58:59:EB:9A:6B:CB:A5:FC:3E:67:BE:6B:F4:AF:25:0A:CF:C0:5E:D4:02` | paste into Play Console |
| AAB (upload package) | `dist/viewit-android-arm64-release.aab` | `android-release.sh` `:app:bundleArm64Release` + sign |
| APK (optional install) | `dist/viewit-android-arm64-release.apk` | `android-release.sh` (zipalign -P16 + v2/v3) |
| GitHub release | repo `mihir0209/ViewIt` tag `v0.2.0` | `gh release create` + upload AAB/APK |

The catalog download URLs point at `https://omnia.mihirpatil.co/plugins/*.zip`
(your Pages custom domain) and already resolve with HTTP 200 — plugins do **not**
depend on GitHub Releases.

## Why an upload key?

Google Play uses **Play App Signing** hosted on their side. For a **new app** you
submit the AAB signed with an "upload key"; Google strips it and re-signs the
installable APKs with a separate app-signing key they hold. You must register your
upload certificate once. The keystore is already generated — you only paste its
SHA-256 fingerprint.

> Do not lose `~/.android/viewit-upload.jks`. If it is lost, you cannot update the
> app without re-registering keys in Play Console. Back it up to a trusted vault.

## The manual steps (needs your Google account + $25)

### 1. Create the Play Console developer account
1. Go to https://play.google.com/console and sign in with a Google account you
   keep (the app will be tied to it).
2. Choose **Create app** → accept the Developer Distribution Agreement.
3. Pay the one-time **$25** developer registration fee if your account hasn't.
4. Set your developer name, contact email, and the **Privacy Policy** URL to the
   hosted `docs/PRIVACY.md` or the site you publish it to.

### 2. Create the app
1. **Create app** → App name: **ViewIt** · default language: English · app or game:
   **App** · Free or paid: `Free`.
2. Complete the **Dashboard** checklists for the store listing, content rating,
   target audience, and data safety (copy from `docs/STORE-LISTING.md` and
   `docs/PRIVACY.md`).

### 3. Register the upload key
In the Play Console for ViewIt:
1. **Setup → App integrity → App signing**.
2. Choose **Upload key** → **Export and upload a key from Java keystore** (this is
   the recommended path for a directly-managed key).
3. Paste the fingerprint above, or upload `~/.android/viewit-upload.jks` when
   prompted (see `viewit-upload.env` for the store/key password).
4. Save. Google records your upload cert; from now on you sign AABs with this key.

### 4. Upload the AAB
1. **Production → Create new release**.
2. Upload the AAB: `dist/viewit-android-arm64-release.aab`.
3. Fill the release notes from the GitHub release notes (`CHANGELOG.md` / `v0.2.0`).
4. Save → Review → **Start rollout to Production**.

### 5. About the Play App Signing key
Once your app is live, Google holds the **app-signing key** and upgrades from its
default to Play App Signing automatically (new apps use it by default). If you
ever need to sign a download directly (e.g., a side-loaded APK), that key is
managed in **Setup → App integrity → App signing** in the console; do not upload
it anywhere.

## Verify before upload

```bash
bash scripts/quality-gate.sh                      # 10/10 must pass
bash scripts/android-release.sh                   # produces dist/**AAB + APK
bash scripts/check-catalog.sh https://omnia.mihirpatil.co   # ALL PLUGINS OK
apksigner verify --verbose dist/viewit-android-arm64-release.aab
```

## One-line summary

Everything is generated and built; the only remaining user action is the Play
Console account + the ~$25 fee + pasting the upload fingerprint above and
uploading `dist/viewit-android-arm64-release.aab`.
