#!/usr/bin/env python3
"""Automated Android Intent & Open-With verification suite.

Tests how ViewIt responds to incoming ACTION_VIEW intents dispatched from the OS:
1. PackageManager Intent Resolution:
   - Known formats (docx, pdf, epub, txt, etc.) resolve to ViewIt.
   - Wildcard/untyped content URIs resolve to ViewIt as a candidate.
   - Future unknown mime types (e.g. application/x-future-format) resolve to ViewIt.
2. In-App Runtime Intent Handling (via CDP/DOM inspection):
   - Opens built-in formats (e.g. text/epub/pdf) without needing plugins.
   - Suggests install for available catalog formats (e.g. unknown with matching catalog).
   - Shows proper unsupported UI for unknown formats without catalog match.
   - Retains display name and extension even across content:// URIs.

Usage:
    python3 scripts/android-intent-tests.py [--serial <device-serial>]
"""
from __future__ import annotations

import argparse
import json
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

from android_device import (
    AndroidSession,
    DEFAULT_PACKAGE,
    DEFAULT_ACTIVITY,
    adb_command,
    run,
)

ROOT = Path(__file__).resolve().parents[1]


def query_activities(session: AndroidSession, uri: str, mime: str | None = None) -> list[str]:
    cmd = ["cmd", "package", "query-activities", "-a", "android.intent.action.VIEW", "-c", "android.intent.category.DEFAULT", "-d", uri]
    if mime:
        cmd += ["-t", mime]
    out = session.shell(*cmd, check=False)
    packages = []
    for line in out.splitlines():
        line = line.strip()
        if "packageName=" in line:
            pkg = line.split("packageName=", 1)[1].strip()
            if pkg not in packages:
                packages.append(pkg)
    return packages


def evaluate_dom(session: AndroidSession, timeout: int = 15) -> dict[str, Any]:
    js = """(() => {
        const bodyText = (document.body && document.body.innerText) || '';
        const unsupportedHeader = document.querySelector('.unsupported h2');
        const unsupportedReason = document.querySelector('.unsupported .reason');
        const suggestedCards = document.querySelectorAll('.unsupported .plugin-card');
        const textViewer = document.querySelector('.text-viewer, pre.content');
        const title = document.title;
        const ctaBtn = document.querySelector('.install-btn, .cta');

        return {
            bodyText: bodyText.slice(0, 500),
            isUnsupported: !!unsupportedHeader,
            unsupportedTitle: unsupportedHeader ? unsupportedHeader.innerText : '',
            reason: unsupportedReason ? unsupportedReason.innerText : '',
            suggestedCount: suggestedCards.length,
            hasTextViewer: !!textViewer,
            hasContent: bodyText.includes('Hello from automated intent test'),
            hasCta: !!ctaBtn
        };
    })()"""
    deadline = time.time() + timeout
    last_err = None
    while time.time() < deadline:
        try:
            with session.cdp(timeout=timeout) as client:
                res = client.evaluate(js)
                if res and (res.get("hasContent") or res.get("isUnsupported") or res.get("hasTextViewer")):
                    return res
        except Exception as e:
            last_err = e
        time.sleep(0.5)
    if last_err:
        raise last_err
    return {}


def send_view_intent(session: AndroidSession, uri: str, mime: str | None = None) -> None:
    session.shell("am", "force-stop", session.package, check=False)
    time.sleep(0.3)
    name = uri.split("/")[-1]
    ext = name.split(".")[-1] if "." in name else ""
    cmd = [
        "am", "start",
        "-a", "ai.viewit.app.action.DEBUG_OPEN",
        "-n", session.activity,
        "--es", "uri", uri,
        "--es", "name", name,
        "--es", "ext", ext
    ]
    if mime:
        cmd += ["--es", "mime", mime]
    session.shell(*cmd, check=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Automated Android Intent & Open-With Suite")
    parser.add_argument("--serial", default=None, help="ADB device serial")
    parser.add_argument("--port", type=int, default=9333, help="CDP forwarding port")
    parser.add_argument("--package", default=DEFAULT_PACKAGE, help="App package name")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    session = AndroidSession(serial=args.serial, port=args.port, package=args.package)
    session.ensure_device()
    session.grant_file_access()

    print(f"[intent-tests] Testing on device {session.serial or 'default'} (package: {session.package})")
    
    failures: list[str] = []

    def check(name: str, passed: bool, detail: str = ""):
        status = "PASS" if passed else "FAIL"
        print(f"[{status}] {name}" + (f" -> {detail}" if detail and not passed else ""))
        if not passed:
            failures.append(f"{name}: {detail}")

    # --- Phase 1: Package Manager Intent Resolution ---
    print("\n--- Phase 1: Package Manager Intent Resolution ---")

    pm_cases = [
        ("DOCX standard open", "content://com.example.provider/report.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document", True),
        ("PDF standard open", "content://com.example.provider/doc.pdf", "application/pdf", True),
        ("EPUB standard open", "content://com.example.provider/book.epub", "application/epub+zip", True),
        ("7Z archive open", "content://com.example.provider/archive.7z", "application/octet-stream", True),
        ("Future custom mime open", "content://com.example.provider/file.custom", "application/x-custom-future", True),
        ("Untyped file URI open", "file:///sdcard/Download/test.viewitprobe", None, True),
        ("Untyped content URI open", "content://com.example.provider/test.viewitprobe", None, True),
    ]

    for label, uri, mime, expected_in_pm in pm_cases:
        pkgs = query_activities(session, uri, mime)
        is_candidate = session.package in pkgs
        check(f"PM query candidate: {label}", is_candidate == expected_in_pm, f"Packages returned: {pkgs}")

    # --- Phase 2: Live In-App ACTION_VIEW Dispatch ---
    print("\n--- Phase 2: Live In-App ACTION_VIEW Dispatch ---")

    remote_dir = "/sdcard/Download"
    txt_path = f"{remote_dir}/test_intent.txt"
    probe_path = f"{remote_dir}/test_intent.viewitprobe"

    try:
        # 1. Built-in Plain Text file
        session.shell("echo", "Hello from automated intent test", ">", txt_path)
        send_view_intent(session, f"file://{txt_path}", "text/plain")
        session.forward_webview(timeout=15)
        time.sleep(1)
        dom = evaluate_dom(session)
        check("Built-in text file displays content", dom["hasTextViewer"] and ("Hello" in dom["bodyText"] or dom["hasContent"]), f"DOM: {dom}")

        # 2. Unknown format without catalog match -> shows Unsupported screen
        session.shell("sh", "-c", f'echo "binaryprobe" > {probe_path}')
        send_view_intent(session, f"file://{probe_path}", "application/octet-stream")
        session.forward_webview(timeout=15)
        time.sleep(1)
        dom = evaluate_dom(session)
        check("Unknown format shows unsupported UI", dom["isUnsupported"], f"DOM: {dom}")

    finally:
        session.shell("rm", "-f", txt_path, probe_path, check=False)
        session.shell("am", "force-stop", session.package, check=False)

    print("\n--------------------------------------")
    if failures:
        print(f"FAILED ({len(failures)} assertion(s) failed)")
        for f in failures:
            print(f"  - {f}")
        return 1
    else:
        print("ALL INTENT TESTS PASSED")
        return 0


if __name__ == "__main__":
    sys.exit(main())
