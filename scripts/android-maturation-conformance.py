#!/usr/bin/env python3
"""Exercise Android storage, intent, lifecycle, UI, memory, and cleanup gates."""

from __future__ import annotations

import argparse
import base64
import hashlib
import importlib.util
import json
import os
import re
import sys
import time
from pathlib import Path
from typing import Any

from android_device import AndroidSession, run


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_APK = ROOT / "dist/viewit-android-arm64-release.apk"
DEFAULT_FIXTURES = ROOT / "build/testing_viewit"
DEFAULT_OUTPUT = ROOT / "build/android-maturation/conformance"
MEDIA_SPEC = importlib.util.spec_from_file_location(
    "android_media_worker_conformance",
    ROOT / "scripts/android-media-worker-conformance.py",
)
MEDIA = importlib.util.module_from_spec(MEDIA_SPEC)
assert MEDIA_SPEC.loader
sys.modules[MEDIA_SPEC.name] = MEDIA
MEDIA_SPEC.loader.exec_module(MEDIA)
DEFAULT_PLUGIN = MEDIA.DEFAULT_PLUGIN


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", default=os.environ.get("ADB_SERIAL") or os.environ.get("ANDROID_SERIAL", ""))
    parser.add_argument("--port", type=int, default=int(os.environ.get("CDP_PORT", "9365")))
    parser.add_argument("--apk", type=Path, default=DEFAULT_APK)
    parser.add_argument("--fixtures", type=Path, default=DEFAULT_FIXTURES)
    parser.add_argument("--plugin", type=Path, default=DEFAULT_PLUGIN)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--no-install", action="store_true")
    parser.add_argument("--confirm-device-reset", action="store_true")
    return parser.parse_args()


def media_id(session: AndroidSession, display_name: str) -> str:
    rows = session.shell(
        "content", "query", "--uri", "content://media/external/video/media",
        "--projection", "_id:_display_name",
    )
    matches = [
        match.group(1)
        for line in rows.splitlines()
        if f"_display_name={display_name}" in line
        for match in [re.search(r"_id=(\d+)", line)]
        if match
    ]
    if not matches:
        raise RuntimeError(f"MediaStore did not index {display_name}: {rows}")
    return matches[-1]


def open_view(session: AndroidSession, uri: str, mime: str, *, cold: bool) -> None:
    if cold:
        session.shell("am", "force-stop", session.package, check=False)
    session.shell(
        "am", "start", "-W", "-a", "android.intent.action.VIEW", "-d", uri,
        "-t", mime, "-f", "1", "-n", session.activity,
    )
    time.sleep(2)


def dom_state(session: AndroidSession) -> dict[str, Any]:
    with session.cdp(timeout=30) as cdp:
        return cdp.evaluate("""(() => ({
          text: (document.body?.innerText || '').slice(0, 1000),
          media: Boolean(document.querySelector('.media-viewer')),
          video: Boolean(document.querySelector('video')),
          error: document.querySelector('pre.error, .error-text')?.textContent || '',
          width: innerWidth,
          scrollWidth: document.documentElement.scrollWidth,
          height: innerHeight,
          scrollHeight: document.documentElement.scrollHeight,
          controls: [...document.querySelectorAll('button, input[type="range"], select, a[href], [role="button"]')].filter(control => {
            const style = getComputedStyle(control); const box = control.getBoundingClientRect();
            return style.display !== 'none' && style.visibility !== 'hidden' && box.width > 0 && box.height > 0;
          }).map(control => {
            const box = control.getBoundingClientRect();
            return { label: control.getAttribute('aria-label') || control.textContent || control.getAttribute('type') || '', width: box.width, height: box.height };
          }),
        }))()""")


def diagnostics(session: AndroidSession, action: str = "state") -> dict[str, Any]:
    with session.cdp(timeout=30) as cdp:
        return json.loads(cdp.evaluate(f"AndroidBridge.androidMaturationDiagnostics({json.dumps(action)})"))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def night_mode_value(raw: str) -> str:
    lower = raw.lower()
    for value in ("custom_schedule", "custom_bedtime", "auto", "yes", "no"):
        if value in lower:
            return value
    raise ValueError(f"unrecognized night mode: {raw}")


def run_worker_fixture(session: AndroidSession, fixture: Path, ext: str, request_id: str) -> dict[str, Any]:
    payload = base64.b64encode(fixture.read_bytes()).decode()
    with session.cdp(timeout=700) as cdp:
        result = cdp.evaluate(MEDIA.callback_expression("probeMediaWorker", [payload, ext], request_id))
    if result["terminal"].get("event") != "complete":
        raise RuntimeError(f"worker fixture failed: {result}")
    MEDIA.validate_progress(result["progress"])
    output = result["terminal"]["result"]
    mimes = {stream.get("mime") for stream in output.get("streams", [])}
    if output.get("sizeBytes", 0) <= 0 or "video/avc" not in mimes:
        raise RuntimeError(f"silent video did not produce non-empty AVC output: {output}")
    return result


def pss(session: AndroidSession, process: str) -> int:
    return MEDIA.parse_total_pss(session.shell("dumpsys", "meminfo", process, check=False))


def main() -> int:
    args = parse_args()
    if not args.confirm_device_reset:
        raise SystemExit("--confirm-device-reset is required; this test installs/clears the app and changes temporary device settings")
    required = [args.fixtures / name for name in ("sample.mp4", "sample.avi")] + [args.plugin]
    missing = [str(path) for path in required if not path.is_file()]
    if missing:
        raise SystemExit(f"missing fixtures: {', '.join(missing)}")
    output = args.output.resolve()
    if (ROOT / "build").resolve() not in output.parents:
        raise SystemExit("--output must be under build/")
    output.mkdir(parents=True, exist_ok=True)

    session = AndroidSession(args.serial, args.port)
    session.ensure_device()
    original = {
        "fontScale": session.shell("settings", "get", "system", "font_scale"),
        "accelerometerRotation": session.shell("settings", "get", "system", "accelerometer_rotation"),
        "userRotation": session.shell("settings", "get", "system", "user_rotation"),
        "uiMode": session.shell("cmd", "uimode", "night", check=False),
    }
    original_night_mode = night_mode_value(original["uiMode"])
    remote_video = "/sdcard/Download/viewit-maturation.mp4"
    report: dict[str, Any] = {
        "device": {},
        "settingsBefore": original,
        "artifacts": {
            "apk": str(args.apk.resolve()), "apkSha256": sha256(args.apk), "installedByRunner": not args.no_install,
            "plugin": str(args.plugin.resolve()), "pluginSha256": sha256(args.plugin),
        },
        "cases": {},
    }
    try:
        if not args.no_install:
            session.install(args.apk)
        session.clear_app()
        MEDIA.install_local_plugin(session, args.plugin)
        report["device"] = session.profile()
        run(session.adb + ["push", str(args.fixtures / "sample.mp4"), remote_video])
        session.shell("am", "broadcast", "-a", "android.intent.action.MEDIA_SCANNER_SCAN_FILE", "-d", f"file://{remote_video}")
        time.sleep(2)
        uri = f"content://media/external/video/media/{media_id(session, Path(remote_video).name)}"

        open_view(session, uri, "application/pdf", cold=True)
        cold_pid = session.shell("pidof", session.package)
        cold = dom_state(session)
        if not cold["media"] or cold["error"]:
            raise RuntimeError(f"cold content URI/MIME mismatch did not route by readable media: {cold}")

        open_view(session, uri, "text/plain", cold=False)
        warm_pid = session.shell("pidof", session.package)
        warm = dom_state(session)
        if warm_pid != cold_pid or not warm["media"] or warm["error"]:
            raise RuntimeError(f"warm ACTION_VIEW did not reuse the process and media viewer: {cold_pid} -> {warm_pid}, {warm}")
        report["cases"]["contentUriIntents"] = {"uri": uri, "coldPid": cold_pid, "warmPid": warm_pid, "cold": cold, "warm": warm}

        with session.cdp(timeout=30) as cdp:
            clicked = cdp.evaluate("""(() => {
              const option = [...document.querySelectorAll('button, [role="button"], .option')]
                .find(element => element.textContent?.includes('Native Android player'));
              if (!option) return false; option.click(); return true;
            })()""")
        if not clicked:
            raise RuntimeError("native media runtime option was not available")
        time.sleep(2)
        player_pid = session.shell("pidof", session.package)
        rendered = dom_state(session)
        resumed = session.shell("dumpsys", "activity", "activities", check=False)
        native_activity = "ai.viewit.app/.VideoPlayerActivity" in resumed or "ai.viewit.app.VideoPlayerActivity" in resumed
        chooser_open = "Choose how ViewIt should handle" in rendered["text"]
        if chooser_open or (not rendered["video"] and not native_activity):
            raise RuntimeError(f"media runtime did not produce rendered video/native activity: {rendered}")
        rendered_undersized = [
            control for control in rendered["controls"] if control["width"] < 44 or control["height"] < 44
        ]
        if rendered_undersized:
            raise RuntimeError(f"rendered media controls below 44px touch target: {rendered_undersized}")
        if native_activity:
            session.shell("input", "keyevent", "4")
            time.sleep(1)
        report["cases"]["renderedMedia"] = {
            "pid": player_pid, "videoElement": rendered["video"], "nativeActivity": native_activity,
        }

        session.shell("settings", "put", "system", "accelerometer_rotation", "0")
        session.shell("settings", "put", "system", "user_rotation", "1")
        time.sleep(2)
        rotated = dom_state(session)
        if not rotated["media"] or rotated["error"]:
            raise RuntimeError(f"viewer did not survive rotation: {rotated}")
        report["cases"]["rotation"] = rotated

        session.shell("cmd", "uimode", "night", "yes")
        session.shell("settings", "put", "system", "font_scale", "1.3")
        open_view(session, uri, "video/mp4", cold=True)
        accessible = dom_state(session)
        if accessible["scrollWidth"] > accessible["width"] + 2 or accessible["error"]:
            raise RuntimeError(f"large-font dark UI overflow/error: {accessible}")
        undersized = [control for control in accessible["controls"] if control["width"] < 44 or control["height"] < 44]
        if undersized:
            raise RuntimeError(f"interactive chrome below 44px touch target: {undersized}")
        report["cases"]["darkLargeFont"] = accessible

        before_restart = session.shell("pidof", session.package)
        session.shell("am", "force-stop", session.package)
        open_view(session, uri, "video/mp4", cold=False)
        after_restart = session.shell("pidof", session.package)
        restarted = dom_state(session)
        if before_restart == after_restart or not restarted["media"]:
            raise RuntimeError(f"main process restart did not restore content: {before_restart} -> {after_restart}, {restarted}")
        report["cases"]["processRestart"] = {"beforePid": before_restart, "afterPid": after_restart, "dom": restarted}

        diagnostics(session, "cleanup")
        worker_samples = []
        baseline_main = pss(session, session.package)
        for index in range(5):
            worker_samples.append(run_worker_fixture(session, args.fixtures / "sample.avi", "avi", f"maturation-{index}"))
        final_main = pss(session, session.package)
        final_worker = pss(session, f"{session.package}:media_worker")
        if final_main > baseline_main + 24 * 1024:
            raise RuntimeError(f"main-process PSS drift exceeded 24 MiB: {baseline_main} -> {final_main} KB")
        report["cases"]["workerSoak"] = {
            "iterations": len(worker_samples), "mainPssBeforeKb": baseline_main,
            "mainPssAfterKb": final_main, "workerPssKb": final_worker,
        }
        with session.cdp(timeout=30) as cdp:
            busy = cdp.evaluate(MEDIA.callback_expression("probeMediaWorkerBusy", [], "maturation-busy"))
        if busy["terminal"].get("event") != "complete" or "busy" not in busy["terminal"].get("result", {}).get("error", "").lower():
            raise RuntimeError(f"concurrent worker request was not rejected as busy: {busy}")
        report["cases"]["workerBusy"] = busy

        cancellation_cycles = []
        for index in range(3):
            old_pid = session.shell("pidof", f"{session.package}:media_worker", check=False)
            if not old_pid:
                run_worker_fixture(session, args.fixtures / "sample.avi", "avi", f"maturation-cycle-warmup-{index}")
                old_pid = session.shell("pidof", f"{session.package}:media_worker")
            with session.cdp(timeout=30) as cdp:
                cancelled = cdp.evaluate(
                    MEDIA.callback_expression("probeMediaWorkerCancellation", [5000], f"maturation-cancel-{index}")
                )
            if cancelled["terminal"].get("event") != "error" or "cancel" not in cancelled["terminal"].get("error", "").lower():
                raise RuntimeError(f"cancellation cycle {index} did not report cancellation: {cancelled}")
            deadline = time.time() + 10
            while time.time() < deadline and session.shell("pidof", f"{session.package}:media_worker", check=False) == old_pid:
                time.sleep(0.1)
            rebound = run_worker_fixture(session, args.fixtures / "sample.avi", "avi", f"maturation-rebind-{index}")
            new_pid = session.shell("pidof", f"{session.package}:media_worker")
            if not old_pid or not new_pid or old_pid == new_pid:
                raise RuntimeError(f"cancellation cycle {index} did not replace worker: {old_pid} -> {new_pid}")
            cancellation_cycles.append({"oldPid": old_pid, "newPid": new_pid, "cancelled": cancelled, "rebound": rebound})
        report["cases"]["workerCancellationCycles"] = cancellation_cycles

        state_before_cleanup = diagnostics(session)
        state_after_cleanup = diagnostics(session, "cleanup")
        if state_after_cleanup.get("files"):
            raise RuntimeError(f"maturation cache cleanup left files: {state_after_cleanup}")
        report["cases"]["cacheCleanup"] = {"before": state_before_cleanup, "after": state_after_cleanup}
    finally:
        run(session.adb + ["forward", "--remove", f"tcp:{args.port}"], check=False)
        session.shell("am", "force-stop", session.package, check=False)
        session.shell("rm", "-f", "/data/local/tmp/viewit-media-conformance.zip", check=False)
        session.clear_app()
        if not args.no_install:
            run(session.adb + ["uninstall", session.package], check=False)
        session.shell("settings", "put", "system", "font_scale", original["fontScale"], check=False)
        session.shell("settings", "put", "system", "accelerometer_rotation", original["accelerometerRotation"], check=False)
        session.shell("settings", "put", "system", "user_rotation", original["userRotation"], check=False)
        session.shell("cmd", "uimode", "night", original_night_mode, check=False)
        session.shell("rm", "-f", remote_video, check=False)
        session.shell("am", "broadcast", "-a", "android.intent.action.MEDIA_SCANNER_SCAN_FILE", "-d", f"file://{remote_video}", check=False)

    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"[android-maturation] PASS: {output / 'report.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
