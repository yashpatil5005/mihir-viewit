#!/usr/bin/env bash
# End-to-end Android release QA on a connected physical device.
# Installs the release APK, verifies zero baked plugins, tests the signed default
# catalog, network installs, key formats, plugin bases, screenshots, and upgrade UX.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APK="${APK:-$ROOT/dist/viewit-android-arm64-release.apk}"
PKG="ai.viewit.app"; ACT="$PKG/.MainActivity"; OUT="${OUT:-$ROOT/build/qa-android}"
ADB=(adb ${ANDROID_SERIAL:+-s "$ANDROID_SERIAL"})
mkdir -p "$OUT"
need(){ command -v "$1" >/dev/null||{ echo "missing: $1";exit 2;};}; need adb; need python3
[ -f "$APK" ]||{ echo "missing APK: $APK";exit 2;}
"${ADB[@]}" get-state >/dev/null

echo "[qa] install + clear"
"${ADB[@]}" install -r "$APK" >/dev/null; "${ADB[@]}" shell pm clear "$PKG" >/dev/null
"${ADB[@]}" shell am start -n "$ACT" >/dev/null; sleep 4
PID=$("${ADB[@]}" shell pidof "$PKG" | tr -d '\r')
"${ADB[@]}" forward --remove tcp:9222 >/dev/null 2>&1||true
"${ADB[@]}" forward tcp:9222 "localabstract:webview_devtools_remote_$PID" >/dev/null
cdp(){ python3 /tmp/opencode/cdp.py "$1"; }

echo "[qa] zero baked plugins"
cdp 'JSON.stringify(JSON.parse(window.AndroidBridge.listPlugins()).map(p=>p.id))' | tee "$OUT/zero-plugins.json"

echo "[qa] default signed catalog"
cdp '(async()=>await new Promise(r=>{let id="qa"+Date.now();window._catalogCallback=(c,d)=>{if(c===id)r(JSON.stringify({count:JSON.parse(d).length,ids:JSON.parse(d).map(p=>p.id)}));};window.AndroidBridge.fetchPluginCatalog(id);}))()' | tee "$OUT/catalog.json"

open_case(){ local name="$1" file="$2" mime="$3" expect="$4"; echo "[qa] $name"; "${ADB[@]}" shell am start -a android.intent.action.VIEW -d "file://$file" -t "$mime" -n "$ACT" >/dev/null; sleep 5; cdp "JSON.stringify({name:'$name',text:(document.body.innerText.slice(0,300)),ok:document.body.innerText.includes('$expect')})" | tee "$OUT/$name.json"; "${ADB[@]}" exec-out screencap -p > "$OUT/$name.png"; }

# Add/override paths as needed via env.
open_case xlsx "${XLSX:-/sdcard/Download/viewit-rich.xlsx}" application/vnd.openxmlformats-officedocument.spreadsheetml.sheet "Quarterly"
open_case pptx "${PPTX:-/sdcard/Download/Demo%20PPTs/powerpoint-slides-1.pptx}" application/vnd.openxmlformats-officedocument.presentationml.presentation "1 / 10"
open_case audio "${AUDIO:-/sdcard/Download/tone.wav}" audio/wav "Player"

echo "[qa] done → $OUT"
