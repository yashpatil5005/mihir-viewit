#!/usr/bin/env bash
# Reproducible Android smoke for the optional Office OOXML document runtime.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APK="${APK:-$ROOT/dist/viewit-android-universal-debug.apk}"
PKG="ai.viewit.app"
CATALOG_PORT="${CATALOG_PORT:-8888}"
RESET_APP_DATA="${RESET_APP_DATA:-1}"
ADB_SERIAL="${ADB_SERIAL:-${ANDROID_SERIAL:-}}"
ADB=(adb)
if [[ -n "$ADB_SERIAL" ]]; then
  ADB=(adb -s "$ADB_SERIAL")
fi

DOCX="${DOCX:-/sdcard/Download/viewit-sample.docx}"
XLSX="${XLSX:-/sdcard/Download/viewit-sample.xlsx}"
PPTX="${PPTX:-/sdcard/Download/viewit-sample.pptx}"

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 2
  fi
}

forward_webview() {
  local pid
  for _ in {1..30}; do
    pid="$(${ADB[@]} shell pidof "$PKG" | tr -d '\r' || true)"
    if [[ -n "$pid" ]]; then
      "${ADB[@]}" forward --remove tcp:9222 >/dev/null 2>&1 || true
      "${ADB[@]}" forward tcp:9222 "localabstract:webview_devtools_remote_$pid" >/dev/null
      return 0
    fi
    sleep 1
  done
  echo "app process did not start: $PKG" >&2
  return 1
}

launch_file() {
  local path="$1"
  local mime="$2"
  "${ADB[@]}" shell am start \
    -a android.intent.action.VIEW \
    -d "file://$path" \
    -t "$mime" \
    -n "$PKG/.MainActivity" >/dev/null
}

assert_ui_runtime() {
  local ext="$1"
  BU_CDP_URL=http://127.0.0.1:9222 browser-use <<PY
import time
for _ in range(20):
    text = js('document.body.innerText')
    if 'Runtime: Enhanced Office OOXML' in text and 'Rendered by Enhanced Office OOXML.' in text:
        print('$ext UI runtime OK')
        break
    time.sleep(1)
else:
    raise SystemExit('$ext UI runtime banner not found')
PY
}

require adb
require browser-use
require python3

if [[ ! -f "$APK" ]]; then
  echo "APK not found: $APK" >&2
  echo "Build it first with: bash scripts/android-release.sh" >&2
  exit 2
fi

for sample in "$DOCX" "$XLSX" "$PPTX"; do
  "${ADB[@]}" shell ls "$sample" >/dev/null
done

echo "[office-smoke] installing $APK"
"${ADB[@]}" install -r "$APK" >/dev/null
if [[ "$RESET_APP_DATA" == "1" ]]; then
  "${ADB[@]}" shell pm clear "$PKG" >/dev/null
fi
"${ADB[@]}" reverse "tcp:$CATALOG_PORT" "tcp:$CATALOG_PORT" >/dev/null

echo "[office-smoke] serving plugin catalog on 127.0.0.1:$CATALOG_PORT"
(cd "$ROOT/plugins" && python3 -m http.server "$CATALOG_PORT" --bind 127.0.0.1 >/tmp/viewit-plugin-catalog.log 2>&1) &
server_pid=$!
trap 'kill "$server_pid" >/dev/null 2>&1 || true' EXIT

"${ADB[@]}" shell am force-stop "$PKG" >/dev/null
"${ADB[@]}" shell am start -n "$PKG/.MainActivity" >/dev/null
sleep 2
forward_webview

echo "[office-smoke] installing ABI-compatible office-ooxml runtime"
BU_CDP_URL=http://127.0.0.1:9222 browser-use <<'PY'
import json, time

catalog = js('''
(async () => await new Promise((resolve) => {
  const id = `smoke_cat_${Date.now()}`;
  window._catalogCallback = (cbId, data) => {
    if (cbId !== id) return;
    delete window._catalogCallback;
    resolve(JSON.parse(data));
  };
  window.AndroidBridge.fetchPluginCatalog(id);
}))()
''')
manifest = next((p for p in catalog if p['id'] == 'office-ooxml'), None)
if not manifest:
    raise SystemExit(f'office-ooxml missing from ABI-filtered catalog: {catalog}')
print('selected', manifest.get('downloadUrl'), manifest.get('abi'), manifest.get('checksum'))
js("window.AndroidBridge.removePlugin('office-ooxml')")
time.sleep(1)

js('''
window._pluginEvents = [];
window._pluginCallback = (msg) => { window._pluginEvents.push(msg); };
window.AndroidBridge.installPlugin(%s, 'smoke_office_install');
''' % json.dumps(json.dumps(manifest)))

for _ in range(180):
    time.sleep(1)
    installed = json.loads(js('window.AndroidBridge.listPlugins()'))
    match = next((p for p in installed if p['id'] == 'office-ooxml'), None)
    if match and match.get('abi') == manifest.get('abi') and match.get('checksum') == manifest.get('checksum'):
        print('installed', match.get('abi'), match.get('sizeBytes'), match.get('checksum'))
        break
else:
    raise SystemExit(f'office-ooxml install timed out: {js("window._pluginEvents.slice(-5)")}')
PY

sleep 1
forward_webview

render_case() {
  local ext="$1"
  local uri="$2"
  forward_webview
  EXT="$ext" URI="$uri" BU_CDP_URL=http://127.0.0.1:9222 browser-use <<'PY'
import json, os, time

ext = os.environ['EXT']
uri = os.environ['URI']
cb = f'smoke_{ext}'
js('''
window._docResults = window._docResults || {};
window._documentPluginCallback = (msg) => { window._docResults[msg.id] = msg; };
window.AndroidBridge.renderDocumentWithPlugin('office-ooxml', %s, %s, %s);
''' % (json.dumps(uri), json.dumps(ext), json.dumps(cb)))
for _ in range(30):
    time.sleep(1)
    result = js('window._docResults[%s]' % json.dumps(cb))
    if result:
        if result.get('error'):
            raise SystemExit(f'{ext} render failed: {result}')
        kind = result.get('document', {}).get('kind')
        if kind != ext:
            raise SystemExit(f'{ext} render returned kind={kind}')
        print(ext, 'render OK')
        break
else:
    raise SystemExit(f'{ext} render timed out')
PY
}

echo "[office-smoke] checking direct office-ooxml renders"
render_case docx 'file:///sdcard/Download/viewit-sample.docx'
render_case xlsx 'file:///sdcard/Download/viewit-sample.xlsx'
render_case pptx 'file:///sdcard/Download/viewit-sample.pptx'

echo "[office-smoke] checking default-open UI runtime banners"
launch_file "$DOCX" "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
sleep 2
forward_webview
assert_ui_runtime docx

launch_file "$XLSX" "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
sleep 2
forward_webview
assert_ui_runtime xlsx

launch_file "$PPTX" "application/vnd.openxmlformats-officedocument.presentationml.presentation"
sleep 2
forward_webview
assert_ui_runtime pptx

echo "[office-smoke] complete"
