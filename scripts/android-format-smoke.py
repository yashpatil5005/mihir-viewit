#!/usr/bin/env python3
"""Reproducible Android format smoke for the ViewIt base app.

This script performs focused DOM/CDP assertions against the installed ViewIt
app on a connected Android device. It is intentionally dependency-free (Python
stdlib + adb only) so it runs in any environment that already builds the APK.

Requirements:
- `adb` on PATH (or pointed at via ADB_PATH)
- A device (optional `ADB_SERIAL`/`ANDROID_SERIAL`)
- The ViewIt app already installed and 16 KB-verified (run
  `bash scripts/android-release.sh` first)
- The fixture folder present on the device at FIXTURE_DIR (default
  `/sdcard/Download/testing_viewit/`) containing the sample set.

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
import hashlib
import json
import os
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from android_device import (
    AndroidSession,
    CdpClient,
    adb_command,
    run as device_run,
)

PKG = "ai.viewit.app"
ACT = "ai.viewit.app/.MainActivity"
DEFAULT_FIXTURE_DIR = "/sdcard/Download/testing_viewit"
DEFAULT_CDP_PORT = 9333
DEFAULT_DEVICE_OUTPUT_DIR = "/sdcard/Download/viewit_smoke_evidence"
REVIEW_INSTRUCTIONS = """Next visual verification workflow:
1. Import evidence with: python3 scripts/import-android-smoke-evidence.py --serial <device-serial>
2. Open .agent/testing/smokeEvidence/grids/grid-001.png, then grid-002.png, etc.
3. For each 3x3 cell, use .agent/testing/smokeEvidence/grid-metadata.json to map grid/cell/row/col back to file, metrics, and screenshot.
4. Replace .agent/testing/smokeTest.md with grid-by-grid observations and false-positive findings before fixing viewer issues.
"""


def adb_cmd(serial: str | None) -> list[str]:
    return adb_command(serial)


def sh(args: list[str], *, check: bool = True, capture: bool = True) -> str:
    return device_run(args, check=check, capture=capture)


def forward(adb: list[str], port: int) -> None:
    session = AndroidSession(port=port)
    session.adb = adb
    session.forward_webview()


def wait_for_cdp(port: int, timeout: int = 20) -> Any:
    return AndroidSession(port=port).websocket_url(timeout)


def ws_eval(ws_url: str, expr: str, *, timeout: int = 15) -> Any:
    with CdpClient(ws_url, timeout) as client:
        return client.evaluate(expr)


def ws_click_text(ws_url: str, text: str, *, timeout: int = 10) -> bool:
    """Click the first button-like element containing text via CDP."""
    click_js = f"""(() => {{
          const elements = [...document.querySelectorAll('button, [role="button"], .option')];
          const el = elements.find(candidate => candidate.textContent?.includes({json.dumps(text)}));
          if (!el) return false;
          el.scrollIntoView({{ block: 'center' }});
          el.click();
          return true;
        }})()"""
    with CdpClient(ws_url, timeout) as client:
        return bool(client.evaluate(click_js))


DOM_QUERY = """(() => {
  const body = document.body.innerText || '';
  const errEl = document.querySelector('pre.error, .error-text');
  const errText = errEl ? errEl.textContent : '';
  const imgEls = document.querySelectorAll('img');
  const audioEls = document.querySelectorAll('audio');
  const videoEls = document.querySelectorAll('video');
  const tableEls = document.querySelectorAll('table');
  const tdEls = document.querySelectorAll('td,th');
  const pluginBanner = document.querySelector('.plugin-renderer-shell, .plugin-renderer-banner');
  const runtimeChooser = document.body.innerText.includes('Choose how ViewIt should handle');

  // Collect first 5 img srcs for content check
  const imgSrcs = Array.from(imgEls).slice(0, 5).map(i => (i.src || '').slice(0, 120));

  // Collect first 3 audio srcs
  const audioSrcs = Array.from(audioEls).slice(0, 3).map(a => (a.src || '').slice(0, 120));

  return {
    // Viewer elements
    docxViewer: !!document.querySelector('.docx-viewer'),
    xlsxViewer: !!document.querySelector('.xlsx-viewer'),
    pptxRoot: !!document.querySelector('.pptx-root'),
    pptxVanilla: body.includes('PPTX Vanilla Viewer') || !!document.querySelector('.pptx-vanilla-viewer'),
    slideText: !!document.querySelector('.slide-text'),
    epubViewer: !!document.querySelector('.epub-viewer, .epub-fullscreen'),
    epubFullscreen: !!document.querySelector('.epub-fullscreen'),
    mediaViewer: !!document.querySelector('.media-viewer'),
    imageViewer: !!document.querySelector('.image-viewer'),
    fontViewer: !!document.querySelector('.font-viewer'),
    pdfViewer: !!document.querySelector('.pdf-viewer, canvas, .pdf-page'),
    archiveViewer: !!document.querySelector('.archive-viewer, .archive-list, .archive-fallback'),

    // Plugin renderer (office-ooxml plugin takes over docx/pptx/xlsx rendering)
    pluginRenderer: !!pluginBanner,
    pluginBannerText: pluginBanner ? pluginBanner.textContent.slice(0, 200) : '',

    // Media
    audio: audioEls.length > 0,
    audioCount: audioEls.length,
    audioSrcs: audioSrcs,
    video: videoEls.length > 0,
    videoCount: videoEls.length,

    // Images
    imgCount: imgEls.length,
    imgSrcs: imgSrcs,
    renderedImages: imgEls.length,

    // Tables
    tableCount: tableEls.length,
    tableCells: tdEls.length,

    // Text quality
    bodyLen: body.length,
    text: body.slice(0, 2000),

    // Error detection — only actual error elements, not CSS class names
    hasError: errText.length > 0,
    errorText: errText.slice(0, 200),
    hasNotDecoded: body.includes('not decoded') || body.includes('not supported') || body.includes('Unsupported'),
    hasRawHtml: body.includes('<html') || body.includes('<head') || body.includes('<body') || body.includes('<p ') || body.includes('<div ') || body.includes('<guide') || body.includes('<reference '),
    hasPartial: body.includes('Partial') || body.includes('partial'),
    archiveFallback: /Archive contents|zip entries|\\bindex\\/|\\[Content_Types\\]/.test(body) || !!document.querySelector('.archive-viewer'),
    archiveEntryCount: parseInt(document.querySelector('.archive-viewer .meta strong')?.textContent?.match(/(\\d+)/)?.[1] ?? '0', 10),
    archiveDrillButtons: document.querySelectorAll('.archive-viewer .drill').length,
    archivePluginSaveButtons: document.querySelectorAll('.archive-viewer .save').length,
    listedByPlugin: body.includes('listed by'),
    archiveNotice: (document.querySelector('.archive-viewer .notice, .archive-viewer .error')?.textContent ?? '').slice(0, 200),
    hasEncryptedNotice: /encrypt|password-protected|password required|wrong password/i.test(body),

    // Runtime / metadata
    runtimeChooser: runtimeChooser,
    externalOpen: body.includes('Open with another app'),
    nativePlayer: body.includes('Native Android player'),
    nativePlayerBtn: !!([...document.querySelectorAll('button, [role="button"], .option')].find(e => e.textContent?.includes('Native Android player'))),

    // Iframe content (epub/mobi chapter rendering)
    iframeText: (() => {
      try {
        const iframe = document.querySelector('.chapter-frame iframe') || document.querySelector('.epub-viewer iframe') || document.querySelector('iframe[srcdoc]');
        return iframe?.contentDocument?.body?.innerText?.substring(0, 1000) || '';
      } catch { return ''; }
    })(),
    iframeHasRawHtml: (() => {
      try {
        const iframe = document.querySelector('.chapter-frame iframe') || document.querySelector('.epub-viewer iframe') || document.querySelector('iframe[srcdoc]');
        const text = iframe?.contentDocument?.body?.innerText || '';
        return /<html|<head|<body|<guide|<reference|<p\\s/i.test(text);
      } catch { return false; }
    })(),
    iframeHasImage: (() => {
      try {
        const iframe = document.querySelector('.chapter-frame iframe') || document.querySelector('.epub-viewer iframe') || document.querySelector('iframe[srcdoc]');
        const doc = iframe?.contentDocument;
        if (!doc) return false;
        // Check for <img> tags and SVG <image> elements (used for EPUB cover pages)
        const hasImg = (doc.querySelectorAll('img')?.length ?? 0) > 0;
        const hasSvgImage = (doc.querySelectorAll('image')?.length ?? 0) > 0;
        return hasImg || hasSvgImage;
      } catch { return false; }
    })(),
    iframeHasContent: (() => {
      try {
        const iframe = document.querySelector('.chapter-frame iframe') || document.querySelector('.epub-viewer iframe') || document.querySelector('iframe[srcdoc]');
        const doc = iframe?.contentDocument;
        if (!doc) return false;
        // Check for any meaningful content: text, images, SVGs, or srcdoc with data
        const hasText = (doc.body?.innerText || '').trim().length > 0;
        const hasImg = (doc.querySelectorAll('img')?.length ?? 0) > 0;
        const hasSvgImage = (doc.querySelectorAll('image')?.length ?? 0) > 0;
        const hasSvg = (doc.querySelectorAll('svg')?.length ?? 0) > 0;
        const srcdocLen = iframe?.getAttribute('srcdoc')?.length ?? 0;
        return hasText || hasImg || hasSvgImage || hasSvg || srcdocLen > 100;
      } catch { return false; }
    })(),
    strategy: (document.querySelector('.media-viewer .hint')?.textContent || document.querySelector('.hint')?.textContent || '').trim().replace(/^strategy:\\s*/i, ''),
    title: document.title,
    url: location.href,
    viewport: { width: innerWidth, height: innerHeight, devicePixelRatio },
  };
})()"""


# Drives the native compression plugin's per-entry extract (the exact path that
# used to corrupt solid-7z saves) straight from the page: reads the first file
# entry's name from the archive viewer DOM, then calls the plugin bridge and
# returns the sha256 of the extracted bytes for byte-exact comparison.
PREVIEW_SHA_JS = """(async () => {
  const info = (window).__viewitArchiveBridge;
  const source = (window).__viewitArchiveSource;
  const drill = document.querySelector('.archive-viewer .drill[aria-label]');
  if (!drill) return { ok: false, reason: 'no file entries to preview' };
  const entryName = drill.getAttribute('aria-label').replace(/^Open\\s+/, '');
  if (!info || !window.AndroidBridge) {
    if (!window.__TAURI__?.core?.invoke || !source?.uri) {
      return { ok: false, reason: 'no archive extraction bridge' };
    }
    try {
      const payload = await window.__TAURI__.core.invoke('archive_extract', {
        uri: source.uri,
        entryName,
      });
      const bytes = payload instanceof Uint8Array ? payload : new Uint8Array(payload);
      const digest = await crypto.subtle.digest('SHA-256', bytes);
      const hex = Array.from(new Uint8Array(digest)).map((b) => b.toString(16).padStart(2, '0')).join('');
      return { ok: true, entry: entryName, size: bytes.length, sha256: hex };
    } catch (e) {
      return { ok: false, reason: String(e), entry: entryName };
    }
  }
  const id = 'smoke_preview_' + Date.now();
  return await new Promise((resolve) => {
    const prev = window.__viewitBridgeDispatch;
    let settled = false;
    const done = (result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      window.__viewitBridgeDispatch = prev;
      resolve(result);
    };
    const timer = setTimeout(() => done({ ok: false, reason: 'preview timed out', entry: entryName }), 15000);
    window.__viewitBridgeDispatch = (payload) => {
      const handled = prev?.(payload) ?? false;
      if (!payload || payload.id !== id) {
        return handled;
      }
      if (payload.event === 'error') {
        done({ ok: false, reason: payload.error || 'archive request failed', entry: entryName });
        return true;
      }
      payload = payload.result || payload;
      if (!payload.base64) {
        done({ ok: false, reason: payload.error || 'no base64 payload', tooLarge: !!payload.tooLarge, entry: entryName });
        return true;
      }
      try {
        const raw = atob(payload.base64);
        const bytes = new Uint8Array(raw.length);
        for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
        crypto.subtle.digest('SHA-256', bytes).then((digest) => {
          const hex = Array.from(new Uint8Array(digest)).map((b) => b.toString(16).padStart(2, '0')).join('');
          done({ ok: true, entry: entryName, size: bytes.length, sha256: hex });
        }).catch((e) => done({ ok: false, reason: String(e), entry: entryName }));
      } catch (e) {
        done({ ok: false, reason: String(e), entry: entryName });
      }
    };
    window.AndroidBridge.extractPluginArchiveEntryAsync(info.pluginId, info.uri, info.name, entryName, id);
  });
})()"""

# Ground-truth sha256 of the first file entry inside each archive fixture,
# computed once with `7z e` / `tar -xzf` from the committed sample set.
PREVIEW_SHA_GROUND_TRUTH = {
    "sample.7z": "145d5d54aed64d7605e1bfab468e8233f973b160d18ecbdfec5a28c5ab3e7794",  # files/data.json (30 B)
    "sample.zip": "f0d6779ce9dbe8cfb1129e116c931c2a6bddb46b97ad09f48c357f46a65874fd",  # archive-content.txt (23 B)
    "sample.tar.gz": "f0d6779ce9dbe8cfb1129e116c931c2a6bddb46b97ad09f48c357f46a65874fd",  # archive-content.txt (23 B)
}


def verify_preview_sha(ws_url: str) -> dict[str, Any]:
    try:
        result = ws_eval(ws_url, PREVIEW_SHA_JS, timeout=20)
    except Exception as e:  # noqa: BLE001
        return {"ok": False, "reason": f"preview sha eval failed: {e}"}
    return result if isinstance(result, dict) else {"ok": False, "reason": f"unexpected preview sha result: {result!r}"}


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


# Common assertion helpers
def _no_error(m: dict) -> str:
    if m.get("hasError"):
        return f"error element visible: {m.get('errorText', '')[:100]}"
    if m.get("hasNotDecoded"):
        return "'not decoded' text visible in DOM"
    return ""


def _no_archive_fallback(m: dict) -> str:
    return assert_falsey(m.get("archiveFallback"), what="archiveFallback")


def _no_partial(m: dict) -> str:
    return assert_falsey(m.get("hasPartial"), what="hasPartial")


def _has_body(m: dict, min_len: int = 20) -> str:
    return assert_min(m.get("bodyLen"), what="bodyLen", n=min_len)


def _docx_or_plugin(m: dict) -> str:
    if m.get("docxViewer") or m.get("pluginRenderer"):
        return ""
    return "docxViewer or pluginRenderer expected"


def _pptx_or_plugin(m: dict) -> str:
    if m.get("pptxRoot") or m.get("pluginRenderer") or m.get("pptxVanilla"):
        return ""
    return "pptxRoot, pluginRenderer, or pptxVanilla expected"


def _xlsx_or_plugin(m: dict) -> str:
    if m.get("xlsxViewer") or m.get("pluginRenderer"):
        return ""
    return "xlsxViewer or pluginRenderer expected"


def _has_odp_fixture_text(m: dict) -> str:
    text = m.get("text", "")
    if "ViewIt PPTX Test" in text and "Runtime rendering verifies slide text extraction." in text:
        return ""
    return "expected ODP title and body text"


ASSERTIONS: dict[str, list[tuple[str, callable]]] = {
    # --- Office / structured documents ---
    "sample.odt": [
        ("no error", _no_error),
        ("docx viewer", lambda m: assert_truthy(m.get("docxViewer"), what="docxViewer")),
        ("has tables", lambda m: assert_min(m.get("tableCount"), what="tableCount", n=1)),
        ("has body", lambda m: _has_body(m, 100)),
    ],
    "sample.ods": [
        ("no error", _no_error),
        ("xlsx viewer", lambda m: assert_truthy(m.get("xlsxViewer"), what="xlsxViewer")),
        ("has grid cells", lambda m: assert_min(m.get("tableCells"), what="tableCells", n=10)),
        ("has body", lambda m: _has_body(m, 50)),
    ],
    "sample.odp": [
        ("no error", _no_error),
        ("pptx viewer", lambda m: assert_truthy(m.get("pptxRoot"), what="pptxRoot")),
        ("has presentation content", _has_odp_fixture_text),
        ("no archive fallback", _no_archive_fallback),
    ],
    "sample.otp": [
        ("no error", _no_error),
        ("pptx viewer", lambda m: assert_truthy(m.get("pptxRoot"), what="pptxRoot")),
        ("has presentation content", _has_odp_fixture_text),
        ("no archive fallback", _no_archive_fallback),
    ],
    "sample.rtf": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 20)),
    ],
    "sample.docx": [
        ("no error", _no_error),
        ("docx viewer or plugin", _docx_or_plugin),
        ("has body", lambda m: _has_body(m, 50)),
    ],
    "sample.docm": [
        ("no error", _no_error),
        ("docx viewer or plugin", _docx_or_plugin),
        ("has body", lambda m: _has_body(m, 50)),
    ],
    "sample.dotx": [
        ("no error", _no_error),
        ("docx viewer or plugin", _docx_or_plugin),
        ("has body", lambda m: _has_body(m, 50)),
    ],
    "sample.dotm": [
        ("no error", _no_error),
        ("docx viewer or plugin", _docx_or_plugin),
        ("has body", lambda m: _has_body(m, 50)),
    ],
    "sample.doc": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 20)),
    ],
    "sample.xlsx": [
        ("no error", _no_error),
        ("xlsx viewer", lambda m: assert_truthy(m.get("xlsxViewer"), what="xlsxViewer")),
        ("has grid cells", lambda m: assert_min(m.get("tableCells"), what="tableCells", n=1)),
        ("has body", lambda m: _has_body(m, 20)),
    ],
    "sample.xls": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 20)),
    ],
    "sample.pptx": [
        ("no error", _no_error),
        ("pptx viewer or plugin", _pptx_or_plugin),
        ("has body", lambda m: _has_body(m, 20)),
    ],
    "sample.ppt": [
        ("no error", _no_error),
        ("pptx viewer", lambda m: assert_truthy(m.get("pptxRoot"), what="pptxRoot")),
        ("has slide text", lambda m: assert_truthy(m.get("slideText"), what="slideText")),
        ("no archive fallback", _no_archive_fallback),
    ],
    # --- Apple iWork (synthetic — these should fail on real files) ---
    "sample.pages": [
        ("no error", _no_error),
        ("not archive fallback", _no_archive_fallback),
        ("honest partial iwork notice", lambda m: assert_truthy(m.get("hasPartial"), what="hasPartial")),
        ("has body", lambda m: _has_body(m, 50)),
    ],
    "sample.numbers": [
        ("no error", _no_error),
        ("not archive fallback", _no_archive_fallback),
        ("honest partial iwork notice", lambda m: assert_truthy(m.get("hasPartial"), what="hasPartial")),
        ("has body", lambda m: _has_body(m, 50)),
    ],
    "sample.key": [
        ("no error", _no_error),
        ("not archive fallback", _no_archive_fallback),
        ("honest partial iwork notice", lambda m: assert_truthy(m.get("hasPartial"), what="hasPartial")),
        ("has body", lambda m: _has_body(m, 50)),
    ],
    # --- eBooks ---
    "sample.epub": [
        ("no error", _no_error),
        ("epub viewer", lambda m: assert_truthy(m.get("epubViewer"), what="epubViewer")),
        ("epub chrome text", lambda m: assert_truthy(m.get("epubFullscreen"), what="epubFullscreen")),
        ("no raw html in body", lambda m: assert_falsey(m.get("hasRawHtml"), what="hasRawHtml")),
        ("iframe has content", lambda m: assert_truthy(m.get("iframeHasContent"), what="iframeHasContent")),
        ("no raw html in iframe", lambda m: assert_falsey(m.get("iframeHasRawHtml"), what="iframeHasRawHtml")),
        ("iframe has image", lambda m: assert_truthy(m.get("iframeHasImage"), what="iframeHasImage")),
    ],
    "sample.mobi": [
        ("no error", _no_error),
        ("epub viewer", lambda m: assert_truthy(m.get("epubViewer"), what="epubViewer")),
        ("epub chrome text", lambda m: assert_truthy(m.get("epubFullscreen"), what="epubFullscreen")),
        ("no raw html in body", lambda m: assert_falsey(m.get("hasRawHtml"), what="hasRawHtml")),
        ("iframe has content", lambda m: assert_truthy(m.get("iframeHasContent"), what="iframeHasContent")),
        ("no raw html in iframe", lambda m: assert_falsey(m.get("iframeHasRawHtml"), what="iframeHasRawHtml")),
    ],
    # --- Audio ---
    "sample.mp3": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
        ("audio element", lambda m: assert_truthy(m.get("audio"), what="audio")),
        ("audio has src", lambda m: assert_truthy(m.get("audioSrcs"), what="audioSrcs")),
    ],
    "sample.wav": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
        ("audio element", lambda m: assert_truthy(m.get("audio"), what="audio")),
    ],
    "sample.flac": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
    ],
    "sample.ogg": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
    ],
    "sample.opus": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
    ],
    "sample.m4a": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
    ],
    "sample.aac": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
    ],
    "sample.aif": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
        ("audio element", lambda m: assert_truthy(m.get("audio"), what="audio")),
        ("aiff-wav strategy", lambda m: assert_eq(m.get("strategy"), what="strategy", expected="aiff-wav")),
    ],
    "sample.aiff": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
        ("audio element", lambda m: assert_truthy(m.get("audio"), what="audio")),
        ("aiff-wav strategy", lambda m: assert_eq(m.get("strategy"), what="strategy", expected="aiff-wav")),
    ],
    "sample.wma": [
        ("no error", _no_error),
        ("no plugin error", lambda m: assert_falsey('No plugin installed' in m.get('text',''), what="noPluginError")),
    ],
    # --- Video ---
    "sample.mp4": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
    ],
    "sample.mkv": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
    ],
    "sample.webm": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
    ],
    "sample.mov": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
    ],
    "sample.avi": [
        ("no error", _no_error),
        ("media viewer", lambda m: assert_truthy(m.get("mediaViewer"), what="mediaViewer")),
    ],
    # --- Images ---
    "sample.jpg": [
        ("no error", _no_error),
        ("image viewer", lambda m: assert_truthy(m.get("imageViewer"), what="imageViewer")),
        ("has images", lambda m: assert_min(m.get("imgCount"), what="imgCount", n=1)),
    ],
    "sample.png": [
        ("no error", _no_error),
        ("image viewer", lambda m: assert_truthy(m.get("imageViewer"), what="imageViewer")),
        ("has images", lambda m: assert_min(m.get("imgCount"), what="imgCount", n=1)),
    ],
    "sample.gif": [
        ("no error", _no_error),
        ("image viewer", lambda m: assert_truthy(m.get("imageViewer"), what="imageViewer")),
        ("has images", lambda m: assert_min(m.get("imgCount"), what="imgCount", n=1)),
    ],
    "sample.webp": [
        ("no error", _no_error),
        ("image viewer", lambda m: assert_truthy(m.get("imageViewer"), what="imageViewer")),
        ("has images", lambda m: assert_min(m.get("imgCount"), what="imgCount", n=1)),
    ],
    "sample.bmp": [
        ("no error", _no_error),
        ("image viewer", lambda m: assert_truthy(m.get("imageViewer"), what="imageViewer")),
        ("has images", lambda m: assert_min(m.get("imgCount"), what="imgCount", n=1)),
    ],
    "sample.heic": [
        ("no error", _no_error),
    ],
    "sample.heif": [
        ("no error", _no_error),
    ],
    "sample.avif": [
        ("no error", _no_error),
    ],
    "sample.tif": [
        ("no error", _no_error),
    ],
    "sample.tiff": [
        ("no error", _no_error),
    ],
    "sample.dng": [
        ("no error", _no_error),
    ],
    "sample.nef": [
        ("no error", _no_error),
    ],
    # --- Text / code ---
    "sample.txt": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 20)),
    ],
    "sample.md": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 20)),
    ],
    "sample.json": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 10)),
    ],
    "sample.csv": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 10)),
    ],
    "sample.xml": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 10)),
    ],
    "sample.html": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 10)),
    ],
    "sample.css": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 10)),
    ],
    "sample.js": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 10)),
    ],
    # --- PDF ---
    "sample.pdf": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 10)),
    ],
    # --- Fonts ---
    "sample.ttf": [
        ("no error", _no_error),
        ("font viewer", lambda m: assert_truthy(m.get("fontViewer"), what="fontViewer")),
    ],
    "sample.otf": [
        ("no error", _no_error),
        ("font viewer", lambda m: assert_truthy(m.get("fontViewer"), what="fontViewer")),
    ],
    # --- Archives ---
    "sample.zip": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("has entries", lambda m: assert_min(m.get("archiveEntryCount"), what="archiveEntryCount", n=1)),
        ("has drill buttons", lambda m: assert_min(m.get("archiveDrillButtons"), what="archiveDrillButtons", n=1)),
        ("preview bytes match ground truth", lambda m: assert_eq(m.get("previewSha256"), what="previewSha256", expected=PREVIEW_SHA_GROUND_TRUTH["sample.zip"])),
    ],
    "sample.tar": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("has entries", lambda m: assert_min(m.get("archiveEntryCount"), what="archiveEntryCount", n=1)),
        ("has drill buttons", lambda m: assert_min(m.get("archiveDrillButtons"), what="archiveDrillButtons", n=1)),
    ],
    "sample.gz": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
    ],
    "sample.tar.gz": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("has entries", lambda m: assert_min(m.get("archiveEntryCount"), what="archiveEntryCount", n=1)),
        ("has drill buttons", lambda m: assert_min(m.get("archiveDrillButtons"), what="archiveDrillButtons", n=1)),
        ("preview bytes match ground truth", lambda m: assert_eq(m.get("previewSha256"), what="previewSha256", expected=PREVIEW_SHA_GROUND_TRUTH["sample.tar.gz"])),
    ],
    # --- Compression Universal plugin (precise listings) ---
    "sample.7z": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
        ("has entries", lambda m: assert_min(m.get("archiveEntryCount"), what="archiveEntryCount", n=1)),
        ("has drill buttons", lambda m: assert_min(m.get("archiveDrillButtons"), what="archiveDrillButtons", n=1)),
        ("has save buttons", lambda m: assert_min(m.get("archivePluginSaveButtons"), what="archivePluginSaveButtons", n=1)),
        ("preview bytes match ground truth", lambda m: assert_eq(m.get("previewSha256"), what="previewSha256", expected=PREVIEW_SHA_GROUND_TRUTH["sample.7z"])),
    ],
    "sample.rar": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
        ("has entries", lambda m: assert_min(m.get("archiveEntryCount"), what="archiveEntryCount", n=1)),
        ("has drill buttons", lambda m: assert_min(m.get("archiveDrillButtons"), what="archiveDrillButtons", n=1)),
    ],
    "sample.tar.bz2": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
        ("has entries", lambda m: assert_min(m.get("archiveEntryCount"), what="archiveEntryCount", n=1)),
        ("has drill buttons", lambda m: assert_min(m.get("archiveDrillButtons"), what="archiveDrillButtons", n=1)),
    ],
    "sample.tar.xz": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
        ("has entries", lambda m: assert_min(m.get("archiveEntryCount"), what="archiveEntryCount", n=1)),
        ("has drill buttons", lambda m: assert_min(m.get("archiveDrillButtons"), what="archiveDrillButtons", n=1)),
    ],
    "sample.tar.zst": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
        ("has entries", lambda m: assert_min(m.get("archiveEntryCount"), what="archiveEntryCount", n=1)),
        ("has drill buttons", lambda m: assert_min(m.get("archiveDrillButtons"), what="archiveDrillButtons", n=1)),
    ],
    "sample.tar.lz4": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
        ("has entries", lambda m: assert_min(m.get("archiveEntryCount"), what="archiveEntryCount", n=1)),
        ("has drill buttons", lambda m: assert_min(m.get("archiveDrillButtons"), what="archiveDrillButtons", n=1)),
    ],
    "sample.tar.lzma": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
        ("has entries", lambda m: assert_min(m.get("archiveEntryCount"), what="archiveEntryCount", n=1)),
        ("has drill buttons", lambda m: assert_min(m.get("archiveDrillButtons"), what="archiveDrillButtons", n=1)),
    ],
    "sample.bz2": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
    ],
    "sample.xz": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
    ],
    "sample.zst": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
    ],
    "sample.lz4": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
    ],
    "sample.lzma": [
        ("no error", _no_error),
        ("archive viewer", lambda m: assert_truthy(m.get("archiveViewer"), what="archiveViewer")),
        ("listed by plugin", lambda m: assert_truthy(m.get("listedByPlugin"), what="listedByPlugin")),
    ],
    "sample.7z.enc": [
        ("encrypted notice visible", lambda m: assert_truthy(m.get("hasEncryptedNotice"), what="hasEncryptedNotice")),
    ],
    # --- Calendar / contacts ---
    "sample.ics": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 20)),
    ],
    "sample.vcf": [
        ("no error", _no_error),
        ("has body", lambda m: _has_body(m, 10)),
    ],
}


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--serial", default=os.environ.get("ADB_SERIAL") or os.environ.get("ANDROID_SERIAL", ""))
    p.add_argument("--fixture-dir", default=os.environ.get("FIXTURE_DIR", DEFAULT_FIXTURE_DIR))
    p.add_argument("--port", type=int, default=int(os.environ.get("CDP_PORT", DEFAULT_CDP_PORT)))
    p.add_argument("--apk", default=os.environ.get("APK", ""))
    p.add_argument("--install", action="store_true", help="Reinstall the APK before running the smoke")
    p.add_argument("--settle", type=float, default=float(os.environ.get("SETTLE_SECONDS", "8")), help="Seconds to wait after launching a file")
    p.add_argument("--only", nargs="*", default=None, help="Only run the listed fixtures")
    p.add_argument("--json", default=os.environ.get("SMOKE_JSON", ""), help="Optional path to write JSON results")
    p.add_argument("--device-output-dir", default=os.environ.get("SMOKE_DEVICE_OUTPUT_DIR", DEFAULT_DEVICE_OUTPUT_DIR), help="Device directory for overwritten screenshots and metadata")
    p.add_argument("--no-screenshots", action="store_true", help="Disable device screenshot capture")
    return p.parse_args()


def ensure_device(adb: list[str]) -> None:
    out = sh(adb + ["get-state"], check=False).strip()
    if out != "device":
        raise SystemExit(f"device not ready (adb get-state -> {out!r}). Plug in / authorize device and retry.")


def ensure_fixtures(adb: list[str], fixture_dir: str, names: Iterable[str]) -> None:
    missing = []
    for name in names:
        out = sh(adb + ["shell", "ls", f"{fixture_dir}/{name}"], check=False).strip()
        if "No such file or directory" in out or not out:
            missing.append(name)
    if missing:
        raise SystemExit(
            f"missing {len(missing)} fixture(s) in {fixture_dir}: {', '.join(missing[:10])}{'...' if len(missing) > 10 else ''}. "
            "Push the sample set to the device first."
        )


def open_file(adb: list[str], fixture_dir: str, name: str) -> None:
    sh(adb + ["shell", "am", "force-stop", PKG], check=False)
    command = adb + ["shell", "am", "start", "-a", "android.intent.action.VIEW", "-d", f"file://{fixture_dir}/{name}", "-n", ACT]
    result = sh(command, check=False)
    if "Error:" in result:
        time.sleep(0.5)
        result = sh(command, check=False)
    if "Error:" in result:
        raise RuntimeError(f"could not launch {name}: {result}")


def grant_fixture_file_access(adb: list[str]) -> None:
    """Allow file:// fixture access after app reinstalls.

    The smoke suite launches files from /sdcard/Download. Android resets the
    all-files app-op on uninstall/reinstall, so without this every fixture fails
    before parser/viewer code is exercised.
    """
    sh(adb + ["shell", "appops", "set", PKG, "MANAGE_EXTERNAL_STORAGE", "allow"], check=False)
    sh(adb + ["shell", "appops", "set", PKG, "READ_EXTERNAL_STORAGE", "allow"], check=False)


def shell_quote(value: str) -> str:
    return "'" + value.replace("'", "'\\''") + "'"


def safe_name(index: int, name: str) -> str:
    stem = "".join(c if c.isalnum() or c in ".-_" else "_" for c in name)
    return f"{index:03d}_{stem}"


def prepare_device_output(adb: list[str], output_dir: str, enabled: bool) -> None:
    if not enabled:
        return
    quoted = shell_quote(output_dir)
    sh(adb + ["shell", f"rm -rf {quoted} && mkdir -p {quoted}/screenshots {quoted}/metadata"])


def capture_screenshot(adb: list[str], output_dir: str, stem: str) -> dict[str, Any]:
    remote = f"{output_dir}/screenshots/{stem}.png"
    data = subprocess.check_output(adb + ["exec-out", "screencap", "-p"])
    sha = hashlib.sha256(data).hexdigest()
    proc = subprocess.run(adb + ["shell", "dd", f"of={remote}", "bs=1048576"], input=data, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if proc.returncode != 0:
        raise RuntimeError(proc.stdout.decode(errors="replace"))
    return {"path": remote, "bytes": len(data), "sha256": sha}


def write_device_json(adb: list[str], remote_path: str, value: Any) -> None:
    data = json.dumps(value, indent=2, sort_keys=True).encode()
    proc = subprocess.run(adb + ["shell", "dd", f"of={remote_path}", "bs=1048576"], input=data, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if proc.returncode != 0:
        raise RuntimeError(proc.stdout.decode(errors="replace"))


def shutil_which(cmd: str) -> str | None:
    try:
        import shutil
        return shutil.which(cmd)
    except Exception:  # noqa: BLE001
        return None


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

    grant_fixture_file_access(adb)

    files = list(args.only) if args.only else list(ASSERTIONS.keys())
    ensure_fixtures(adb, args.fixture_dir, files)

    screenshots_enabled = not args.no_screenshots
    prepare_device_output(adb, args.device_output_dir, screenshots_enabled)

    results = []
    overall_failures = 0

    for index, name in enumerate(files, start=1):
        assertions = ASSERTIONS.get(name)
        if not assertions:
            print(f"[smoke] SKIP {name} (no assertions registered)")
            continue
        stem = safe_name(index, name)
        started_at = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        open_file(adb, args.fixture_dir, name)
        time.sleep(args.settle)
        forward(adb, args.port)
        screenshot = None
        try:
            ws_url = wait_for_cdp(args.port)
            metrics = ws_eval(ws_url, DOM_QUERY)
            assert metrics is not None, f"no metrics returned for {name}"

            # Nested archives can expose only a directory at their root. Descend
            # once so file preview/save assertions exercise the first real entry.
            if metrics.get("archiveViewer") and not metrics.get("archivePluginSaveButtons"):
                descended = ws_eval(
                    ws_url,
                    """(() => {
                      const folder = document.querySelector('.archive-viewer .drill.folder');
                      if (!folder || folder.textContent?.includes('..')) return false;
                      folder.click();
                      return true;
                    })()""",
                )
                if descended:
                    time.sleep(0.25)
                    metrics = ws_eval(ws_url, DOM_QUERY)
                    assert metrics is not None, f"no metrics after archive folder navigation for {name}"
                    metrics["archiveFolderDescended"] = True

            # If runtime chooser appeared, click "Native Android player" and re-evaluate
            if metrics.get("runtimeChooser") and metrics.get("nativePlayerBtn"):
                print(f"         [info] runtime chooser detected, clicking 'Native Android player'")
                clicked = ws_click_text(ws_url, "Native Android player")
                if clicked:
                    time.sleep(3)  # Wait for content to load after click
                    metrics = ws_eval(ws_url, DOM_QUERY)
                    assert metrics is not None, f"no metrics after runtime click for {name}"
                    metrics["runtimeClicked"] = True
                else:
                    metrics["runtimeClickFailed"] = True

            if screenshots_enabled:
                screenshot = capture_screenshot(adb, args.device_output_dir, stem)
        except Exception as e:  # noqa: BLE001
            if screenshots_enabled and screenshot is None:
                try:
                    screenshot = capture_screenshot(adb, args.device_output_dir, stem)
                except Exception as capture_err:  # noqa: BLE001
                    screenshot = {"error": str(capture_err)}
            result = {
                "index": index,
                "file": name,
                "fixture_path": f"{args.fixture_dir}/{name}",
                "ok": False,
                "error": str(e),
                "failures": [str(e)],
                "metrics": None,
                "screenshot": screenshot,
                "device_output_dir": args.device_output_dir,
                "started_at": started_at,
                "settle_seconds": args.settle,
            }
            results.append(result)
            if screenshots_enabled:
                write_device_json(adb, f"{args.device_output_dir}/metadata/{stem}.json", result)
            print(f"[smoke] FAIL {name}: {e}")
            overall_failures += 1
            continue

        if name in PREVIEW_SHA_GROUND_TRUTH:
            pv = verify_preview_sha(ws_url)
            metrics["previewSha256"] = pv.get("sha256")
            metrics["previewShaEntry"] = pv.get("entry")
            metrics["previewShaReason"] = pv.get("reason")

        failures = check_metrics(metrics, assertions)
        ok = not failures
        overall_failures += 0 if ok else 1
        result = {
            "index": index,
            "file": name,
            "fixture_path": f"{args.fixture_dir}/{name}",
            "ok": ok,
            "failures": failures,
            "metrics": metrics,
            "screenshot": screenshot,
            "device_output_dir": args.device_output_dir,
            "started_at": started_at,
            "settle_seconds": args.settle,
        }
        results.append(result)
        if screenshots_enabled:
            write_device_json(adb, f"{args.device_output_dir}/metadata/{stem}.json", result)
        if ok:
            print(f"[smoke] PASS {name}")
        else:
            print(f"[smoke] FAIL {name}")
            for f in failures:
                print(f"         - {f}")

    if screenshots_enabled:
        manifest = {
            "package": PKG,
            "activity": ACT,
            "serial": args.serial,
            "fixture_dir": args.fixture_dir,
            "device_output_dir": args.device_output_dir,
            "settle_seconds": args.settle,
            "total": len(results),
            "passed": len(results) - overall_failures,
            "failed": overall_failures,
            "results": results,
            "review_instructions": REVIEW_INSTRUCTIONS.strip().splitlines(),
        }
        write_device_json(adb, f"{args.device_output_dir}/manifest.json", manifest)
        print(f"[smoke] wrote screenshots/metadata to device: {args.device_output_dir}")

    if args.json:
        Path(args.json).parent.mkdir(parents=True, exist_ok=True)
        with open(args.json, "w") as f:
            for r in results:
                f.write(json.dumps(r) + "\n")
        print(f"[smoke] wrote JSONL to {args.json}")

    print()
    print(f"=== Smoke results: {len(results) - overall_failures}/{len(results)} passed, {overall_failures} failed ===")
    if screenshots_enabled:
        print()
        print(REVIEW_INSTRUCTIONS.strip())
    return 0 if overall_failures == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
