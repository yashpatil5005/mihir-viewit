#!/usr/bin/env bash
# Pull the installed base.apk from a connected device and re-verify that the
# actually-installed APK (not just the build artifact) is 16 KB-compatible.
#
# This complements scripts/verify-android-16kb.sh, which only inspects a
# provided path. Verifying the *installed* APK catches cases where a
# post-build signing/zipalign/strip step regresses alignment between the
# build artifact and what the user actually runs.
#
# Usage:
#   bash scripts/verify-installed-apk.sh                       # uses $ANDROID_SERIAL or first device
#   bash scripts/verify-installed-apk.sh RZCW71NK7SN            # explicit serial
#   bash scripts/verify-installed-apk.sh RZCW71NK7SN /tmp/out/base.apk   # custom output path
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PKG="ai.viewit.app"
SERIAL="${1:-${ANDROID_SERIAL:-}}"
ADB=(adb)
if [[ -n "$SERIAL" ]]; then
  ADB=(adb -s "$SERIAL")
fi
OUT="${2:-/tmp/opencode/viewit-installed-verified.apk}"

if ! command -v adb >/dev/null 2>&1; then
  echo "adb not found on PATH" >&2
  exit 2
fi

state="$("${ADB[@]}" get-state 2>/dev/null || true)"
if [[ "$state" != "device" ]]; then
  echo "device not ready (adb get-state -> ${state:-<none>})" >&2
  exit 2
fi

echo "[verify-installed] resolving APK path for ${PKG}"
APK_PATH="$("${ADB[@]}" shell pm path "${PKG}" | tr -d '\r' | sed 's/^package://')"
if [[ -z "$APK_PATH" ]]; then
  echo "package ${PKG} is not installed on the device" >&2
  exit 2
fi
echo "[verify-installed] installed APK: ${APK_PATH}"

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"
"${ADB[@]}" pull "$APK_PATH" "$OUT" >/dev/null
echo "[verify-installed] pulled to ${OUT}"

bash "$ROOT/scripts/verify-android-16kb.sh" "$OUT"
