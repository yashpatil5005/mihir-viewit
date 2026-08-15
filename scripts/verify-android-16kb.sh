#!/usr/bin/env bash
set -euo pipefail

APK="${1:-}"
if [[ -z "$APK" ]]; then
  echo "usage: $0 path/to/app.apk" >&2
  exit 2
fi
if [[ ! -f "$APK" ]]; then
  echo "APK not found: $APK" >&2
  exit 2
fi

ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
NDK="${ANDROID_NDK_HOME:-${NDK_HOME:-$ANDROID_HOME/ndk/26.1.10909125}}"
BUILD_TOOLS=$(ls -d "$ANDROID_HOME/build-tools/"* 2>/dev/null | sort -V | tail -1)
READELF="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-readelf"
ZIPALIGN="$BUILD_TOOLS/zipalign"

if [[ ! -x "$READELF" ]]; then
  echo "llvm-readelf not found: $READELF" >&2
  exit 2
fi
if [[ ! -x "$ZIPALIGN" ]]; then
  echo "zipalign not found: $ZIPALIGN" >&2
  exit 2
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

python3 - "$APK" "$TMP" <<'PY'
import sys, zipfile
apk, tmp = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(apk) as z:
    so_list = [n for n in z.namelist() if n.startswith('lib/arm64-v8a/') and n.endswith('.so')]
    if not so_list:
        raise SystemExit(f'No arm64-v8a .so found in {apk}')
    for n in so_list:
        z.extract(n, tmp)
PY

for LIB in "$TMP"/lib/arm64-v8a/*.so; do
  if [ -f "$LIB" ]; then
    if ! "$READELF" -l "$LIB" | awk '/LOAD/ && $NF != "0x4000" { bad=1; print } END { exit bad }'; then
      echo "ELF LOAD segments are not 16 KB-aligned in $LIB from $APK" >&2
      exit 1
    fi
  fi
done

"$ZIPALIGN" -c -v 4 "$APK" | awk '
  /lib\/arm64-v8a\/.*\.so/ { seen=1; if ($NF != "(OK)" && $NF != "compressed)") bad=1; print }
  /Verification/ { print }
  END { if (!seen || bad) exit 1 }
'

echo "16 KB Android verification passed: $APK"
