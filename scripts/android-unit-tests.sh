#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MOBILE="$ROOT/apps/mobile"
GEN="$MOBILE/src-tauri/gen/android"

export TMPDIR="${VIEWIT_TMPDIR:-$ROOT/build/tmp}"
mkdir -p "$TMPDIR"
export JAVA_TOOL_OPTIONS="${JAVA_TOOL_OPTIONS:-} -Djava.io.tmpdir=$TMPDIR"

if [[ ! -x "$GEN/gradlew" ]]; then
  (cd "$MOBILE" && npm run tauri -- android init)
fi

"$ROOT/scripts/patch-android-mainactivity.sh"
(cd "$GEN" && ./gradlew :app:testArmDebugUnitTest)
