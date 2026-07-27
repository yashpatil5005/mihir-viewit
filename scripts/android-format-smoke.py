#!/usr/bin/env python3
"""Reproducible Android format smoke for the ViewIt base app.

This script performs focused DOM/CDP assertions against the installed ViewIt
app on a connected Android device. It is intentionally dependency-free (Python
stdlib + adb only) so it runs in any environment that already builds the APK.

It does NOT use `browser-use`; the earlier `adb-office-runtime-smoke.sh`
required that CLI which is missing from the build environment.

Requirements:
- `adb` on PATH (or pointed at via ADB_PATH)
- A device (optional `ADB_SERIAL`/`ANDROID_SERIAL`)
- The ViewIt app already installed and 16 KB-verified (run
  `bash scripts/android-release.sh` first)
- The fixture folder present on the device at FIXTURE_DIR (default
  `/sdcard/Download/testing_viewit/`) containing the 58-sample set
  used by `.agent/failures/format-verification.md`.

Exit code:
- 0 if every assertion passes
- 1 if any assertion fails
- 2 if environment prerequisites are missing

Usage:
    python3 scripts/android-format-smoke.py
    ADB_SERIAL=RZCW71NK7SN python3 scripts/android-format-smoke.py
    APK=dist/viewit-android-universal-debug.apk python3 scripts/android-format-smoke.py --install
    FIXTURE_DIR=/sdcard/Download/testing_viewit python3 scripts/android-format-smoke.py
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import socket
import struct
import subprocess
import sys
import time
import urllib.request
from dataclasses import dataclass
from typing import Any, Iterable

PKG = "ai.viewit.app"
ACT = "ai.viewit.app/.MainActivity"
DEFAULT_FIXTURE_DIR = "/sdcard/Download/testing_viewit"
DEFAULT_CDP_PORT = 9333


def adb_cmd(serial: str | None) -> list[str]:
    cmd = ["adb"]
    if serial:
        cmd += ["-s", serial]
    return cmd


def sh(args: list[str], *, check: bool = True, capture: bool = True) -> str:
    res = subprocess.run(
        args,
        check=check,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )
    return res.stdout if capture else ""


def forward(adb: list[str], port: int) -> None:
    # Find the running app pid and forward the WebView devtools socket.
    for _ in range(30):
        pid = sh(adb + ["shell", "pidof", PKG], check=False).strip().replace("\r", "")
        if pid:
            sh(adb + ["forward", "--remove", f"tcp:{port}"], check=False)
            sh(adb + ["forward", f"tcp:{port}", f"localabstract:webview_devtools_remote_{pid}"])
            return
        time.sleep(1)
    raise RuntimeError(f"app process did not start: {PKG}")


def wait_for_cdp(port: int, timeout: int = 20) -> Any:
    deadline = time.time() + timeout
    last_err = ""
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=2) as r:
                pages = json.load(r)
            if pages:
                return pages[0]["webSocketDebuggerUrl"]
        except Exception as e:  # noqa: BLE001
            last_err = str(e)
            time.sleep(0.5)
    raise RuntimeError(f"CDP not ready on http://127.0.0.1:{port}/json: {last_err}")


def _ws_handshake(ws_url: str) -> socket.socket:
    parts = ws_url.split("/")
    hostport = parts[2]
    host, port = hostport.split(":")
    path = "/" + "/".join(parts[3:])
    s = socket.create_connection((host, int(port)), timeout=5)
    key = base64.b64encode(os.urandom(16)).decode()
    req = (
        f"GET {path} HTTP/1.1\r\n"
        f"Host: {hostport}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\n"
        "Sec-WebSocket-Version: 13\r\n\r\n"
    )
    s.sendall(req.encode())
    resp = s.recv(4096)
    if b"101" not in resp.split(b"\r\n", 1)[0]:
        raise RuntimeError(f"WS handshake failed: {resp.decode(errors='replace')[:200]}")
    return s


def _ws_send(s: socket.socket, obj: dict) -> None:
    data = json.dumps(obj).encode()
    mask = os.urandom(4)
    header = bytearray([0x81])  # FIN + text frame
    n = len(data)
    if n < 126:
        header.append(0x80 | n)
    elif n < 65536:
        header += bytes([0x80 | 126]) + struct.pack("!H", n)
    else:
        header += bytes([0x80 | 127]) + struct.pack("!Q", n)
    s.sendall(bytes(header) + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))


def _ws_recv(s: socket.socket) -> dict:
    h = s.recv(2)
    if len(h) < 2:
        raise RuntimeError("WS closed")
    _b1, b2 = h
    n = b2 & 0x7F
    if n == 126:
        n = struct.unpack("!H", s.recv(2))[0]
    elif n == 127:
        n = struct.unpack("!Q", s.recv(8))[0]
    if b2 & 0x80:
        s.recv(4)
    data = b""
    while len(data) < n:
        chunk = s.recv(n - len(data))
        if not chunk:
            raise RuntimeError("WS closed mid-frame")
        data += chunk
    return json.loads(data.decode())


def ws_eval(ws_url: str, expr: str, *, timeout: int = 15) -> Any:
    s = _ws_handshake(ws_url)
    try:
        _ws_send(s, {"id": 1, "method": "Runtime.enable"})
        _ws_recv(s)
        _ws_send(s, {"id": 2, "method": "Runtime.evaluate", "params": {"expression": expr, "returnByValue": True, "awaitPromise": True}})
        deadline = time.time() + timeout
        while time.time() < deadline:
            msg = _ws_recv(s)
            if msg.get("id") == 2:
                result = msg.get("result", {}).get("result", {})
                if result.get("type") == "object" and result.get("subtype") == "error":
                    raise RuntimeError(f"Runtime.evaluate error: {result.get('description')}")
                return result.get("value")
        raise RuntimeError("Runtime.evaluate timed out")
    finally:
        try:
            s.close()
        except Exception:  # noqa: BLE001
            pass


DOM_QUERY = """(() => ({
  text: document.body.innerText.slice(0, 1500),
  pptxRoot: !!document.querySelector('.pptx-root'),
  slideText: !!document.querySelector('.slide-text'),
  docxViewer: !!document.querySelector('.docx-viewer'),
  xlsxViewer: !!document.querySelector('.xlsx-viewer'),
  epubViewer: !!document.querySelector('.epub-viewer'),
  mediaViewer: !!document.querySelector('.media-viewer'),
  audio: !!document.querySelector('audio'),
  audioSrcPrefix: (document.querySelector('audio')?.src || '').slice(0, 40),
  runtimeChooser: /Choose how ViewIt should handle/.test(document.body.innerText),
  externalOpen: /Open with another app/.test(document.body.innerText),
  nativePlayer: /Native Android player/.test(document.body.innerText),
  archiveFallback: /Archive contents|zip entries|\\bindex\\/|\\[Content_Types\\]/.test(document.body.innerText),
  partialNotice: /Partial|partial|not decoded|enhanced Office text extraction/.test(document.body.innerText),
  tableCount: document.querySelectorAll('table').length,
  tableCells: document.querySelectorAll('td,th').length,
  imagePlaceholders: document.querySelectorAll('.docx-image-placeholder, .image-placeholder').length,
  strategy: (document.querySelector('.media-viewer .hint')?.textContent || document.querySelector('.hint')?.textContent || '').trim(),
  bodyLen: document.body.innerText.length,
}))()"""


@dataclass
class Assertion:
    file: str
    description: str
    check: callable  # type: ignore[type-arg]


def assert_truthy(value: Any, *, what: str) -> str:
    return "" if value else f"{what} expected truthy"


def assert_falsey(value: Any, *, what: str) -> str:
    return "" if not value else f"{what} expected falsy, got {value!r}"


def assert_min(value: Any, *, what: str, n: int) -> str:
    return "" if isinstance(value, int) and value >= n else f"{what} expected >= {n}, got {value!r}"


def assert_eq(value: Any, *, what: str, expected: Any) -> str:
    return "" if value == expected else f"{what} expected {expected!r}, got {value!r}"


def check_metrics(metrics: dict, assertions: list[tuple[str, callable]]) -> list[str]:
    failures: list[str] = []
    for label, fn in assertions:
        try:
            err = fn(metrics)
        except Exception as e:  # noqa: BLE001
            err = f"{label} raised: {e}"
        if err:
            failures.append(f"{label}: {err}")
    return failures


ASSERTIONS: dict[str, list[tuple[str, callable]]] = {
    "sample.odt": [
        ("renders in docx viewer", lambda m: assert_truthy(m.get("docxViewer"), what="docxViewer")),
        ("has real tables", lambda m: assert_min(m.get("tableCount"), what="tableCount", n=1)),
        ("has table cells", lambda m: assert_min(m.get("tableCells"), what="tableCells", n=1)),
        ("not archive fallback", lambda m: assert_falsey(m.get("archiveFallback"), what="archiveFallback")),
    ],
    "sample.ods": [
        ("renders in xlsx viewer", lambda m: assert_truthy(m.get("xlsxViewer"), what="xlsxViewer")),
        ("has grid cells", lambda m: assert_min(m.get("tableCells"), what="tableCells", n=1)),
        ("not archive fallback", lambda m: assert_falsey(m.get("archiveFallback"), what="archiveFallback")),
    ],
    "sample.odp": [
        ("renders in pptx viewer", lambda m: assert_truthy(m.get("pptxRoot"), what="pptxRoot")),
        ("has slide text", lambda m: assert_truthy(m.get("slideText"), what="slideText")),
        ("not archive fallback", lambda m: assert_falsey(m.get("archiveFallback"), what="archiveFallback")),
    ],
    "sample.pages": [
        ("renders in docx viewer (not archive)", lambda m: assert_truthy(m.get("docxViewer"), what="docxViewer")),
        ("not archive fallback", lambda m: assert_falsey(m.get("archiveFallback"), what="archiveFallback")),
    ],
    "sample.numbers": [
        ("renders in xlsx viewer (not archive)", lambda m: assert_truthy(m.get("xlsxViewer"), what="xlsxViewer")),
        ("has grid cells", lambda m: assert_min(m.get("tableCells"), what="tableCells", n=1)),
        ("not archive fallback", lambda m: assert_falsey(m.get("archiveFallback"), what="archiveFallback")),
    ],
    "sample.key": [
        ("renders in pptx viewer (not archive)", lambda m: assert_truthy(m.get("pptxRoot"), what="pptxRoot")),
        ("has slide text", lambda m: assert_truthy(m.get("slideText"), what="slideText")),
        ("not archive fallback", lambda m: assert_falsey(m.get("archiveFallback"), what="archiveFallback")),
    ],
    "sample.ppt": [
        ("renders as slide model in pptx viewer", lambda m: assert_truthy(m.get("pptxRoot"), what="pptxRoot")),
        ("has slide text", lambda m: assert_truthy(m.get("slideText"), what="slideText")),
        ("honest partial notice present", lambda m: assert_truthy(m.get("partialNotice"), what="partialNotice")),
        ("not text-only fallback", lambda m: assert_falsey(m.get("docxViewer") and not m.get("pptxRoot"), what="docxViewerTailOnly")),
    ],
    "sample.aif": [
        ("opens media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
        ("in-app audio element", lambda m: assert_truthy(m.get("audio"), what="audio")),
        ("aiff-wav strategy", lambda m: assert_eq(m.get("strategy"), what="strategy", expected="aiff-wav")),
        ("uses blob URL", lambda m: ("" if (m.get("audioSrcPrefix", "") or "").startswith("blob:") else f"audio src expected blob:, got {m.get('audioSrcPrefix')!r}")),
    ],
    "sample.aiff": [
        ("opens media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
        ("in-app audio element", lambda m: assert_truthy(m.get("audio"), what="audio")),
        ("aiff-wav strategy", lambda m: assert_eq(m.get("strategy"), what="strategy", expected="aiff-wav")),
        ("uses blob URL", lambda m: ("" if (m.get("audioSrcPrefix", "") or "").startswith("blob:") else f"audio src expected blob:, got {m.get('audioSrcPrefix')!r}")),
    ],
    "sample.wma": [
        ("opens media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
        ("runtime chooser visible", lambda m: assert_truthy(m.get("runtimeChooser"), what="runtimeChooser")),
        ("native Android player offered", lambda m: assert_truthy(m.get("nativePlayer"), what="nativePlayer")),
        ("external open offered", lambda m: assert_truthy(m.get("externalOpen"), what="externalOpen")),
        ("no broken in-app audio element", lambda m: assert_falsey(m.get("audio"), what="audio")),
    ],
    "sample.mobi": [
        ("opens in epub viewer", lambda m: assert_truthy(m.get("epubViewer"), what="epubViewer")),
        ("has body text", lambda m: assert_min(m.get("bodyLen"), what="bodyLen", n=20)),
        ("title contains 'Geography of Bliss'", lambda m: ("" if "Geography of Bliss" in m.get("text", "") else "expected 'Geography of Bliss' in text")),
    ],
}


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--serial", default=os.environ.get("ADB_SERIAL") or os.environ.get("ANDROID_SERIAL", ""))
    p.add_argument("--fixture-dir", default=os.environ.get("FIXTURE_DIR", DEFAULT_FIXTURE_DIR))
    p.add_argument("--port", type=int, default=int(os.environ.get("CDP_PORT", DEFAULT_CDP_PORT)))
    p.add_argument("--apk", default=os.environ.get("APK", ""))
    p.add_argument("--install", action="store_true", help="Reinstall the APK before running the smoke")
    p.add_argument("--settle", type=float, default=float(os.environ.get("SETTLE_SECONDS", "6")), help="Seconds to wait after launching a file")
    p.add_argument("--only", nargs="*", default=None, help="Only run the listed fixtures (e.g. sample.ppt sample.wma)")
    p.add_argument("--json", default=os.environ.get("SMOKE_JSON", ""), help="Optional path to write JSON results")
    return p.parse_args()


def ensure_device(adb: list[str]) -> None:
    out = sh(adb + ["get-state"], check=False).strip()
    if out != "device":
        raise SystemExit(f"device not ready (adb get-state -> {out!r}). Plug in / authorize device and retry.")


def ensure_fixtures(adb: list[str], fixture_dir: str, names: Iterable[str]) -> None:
    missing = []
    for name in names:
        out = sh(adb + ["shell", "ls", f"{fixture_dir}/{name}"], check=False).strip()
        # Treat device "No such file or directory" / empty output as missing.
        if "No such file or directory" in out or not out:
            missing.append(name)
    if missing:
        raise SystemExit(
            f"missing {len(missing)} fixture(s) in {fixture_dir}: {', '.join(missing[:5])}{'…' if len(missing) > 5 else ''}. "
            "Push the 58-sample set to the device first."
        )


def open_file(adb: list[str], fixture_dir: str, name: str) -> None:
    sh(adb + ["shell", "am", "force-stop", PKG], check=False)
    sh(adb + ["shell", "am", "start", "-a", "android.intent.action.VIEW", "-d", f"file://{fixture_dir}/{name}", "-n", ACT])


def main() -> int:
    if not shutil_which("adb"):
        print("adb not found on PATH", file=sys.stderr)
        return 2
    args = parse_args()
    adb = adb_cmd(args.serial)
    ensure_device(adb)

    if args.install:
        if not args.apk:
            print("--install requires --apk / APK env", file=sys.stderr)
            return 2
        print(f"[smoke] installing {args.apk}")
        sh(adb + ["install", "-r", args.apk])

    files = list(args.only) if args.only else list(ASSERTIONS.keys())
    ensure_fixtures(adb, args.fixture_dir, files)

    results = []
    overall_failures = 0

    for name in files:
        assertions = ASSERTIONS.get(name)
        if not assertions:
            print(f"[smoke] SKIP {name} (no assertions registered)")
            continue
        # Force-stop, launch, attach, evaluate.
        open_file(adb, args.fixture_dir, name)
        time.sleep(args.settle)
        forward(adb, args.port)
        try:
            ws_url = wait_for_cdp(args.port)
            metrics = ws_eval(ws_url, DOM_QUERY)
            assert metrics is not None, f"no metrics returned for {name}"
        except Exception as e:  # noqa: BLE001
            results.append({"file": name, "ok": False, "error": str(e), "metrics": None})
            print(f"[smoke] FAIL {name}: {e}")
            overall_failures += 1
            continue

        failures = check_metrics(metrics, assertions)
        ok = not failures
        overall_failures += 0 if ok else 1
        results.append({"file": name, "ok": ok, "failures": failures, "metrics": metrics})
        if ok:
            print(f"[smoke] PASS {name}")
        else:
            print(f"[smoke] FAIL {name}")
            for f in failures:
                print(f"         - {f}")

    if args.json:
        with open(args.json, "w") as f:
            for r in results:
                f.write(json.dumps(r) + "\n")
        print(f"[smoke] wrote JSONL to {args.json}")

    print()
    print(f"=== Smoke results: {len(results) - overall_failures}/{len(results)} passed, {overall_failures} failed ===")
    return 0 if overall_failures == 0 else 1


def shutil_which(cmd: str) -> str | None:
    try:
        import shutil
        return shutil.which(cmd)
    except Exception:  # noqa: BLE001
        return None


if __name__ == "__main__":
    raise SystemExit(main())
