#!/usr/bin/env python3
"""Run the Android isolated media-worker release gates on one device."""

from __future__ import annotations

import argparse
import base64
import json
import os
import re
import subprocess
import time
from pathlib import Path
from typing import Any

from android_device import AndroidSession, run


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_APK = ROOT / "dist/viewit-android-arm64-release.apk"
PLUGIN_ROOT = ROOT / "plugins/ffmpeg-transcoder"
PLUGIN_VERSION = json.loads((PLUGIN_ROOT / "plugin.json").read_text())["version"]
DEFAULT_PLUGIN = PLUGIN_ROOT / "build" / f"ffmpeg-transcoder-{PLUGIN_VERSION}.zip"
DEFAULT_OUTPUT = ROOT / "build/android-media-worker-conformance"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", default=os.environ.get("ADB_SERIAL") or os.environ.get("ANDROID_SERIAL", ""))
    parser.add_argument("--port", type=int, default=int(os.environ.get("CDP_PORT", "9355")))
    parser.add_argument("--apk", type=Path, default=DEFAULT_APK)
    parser.add_argument("--plugin", type=Path, default=DEFAULT_PLUGIN)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--no-install", action="store_true")
    parser.add_argument("--confirm-device-reset", action="store_true")
    return parser.parse_args()


def validate_progress(values: list[float]) -> None:
    if len(values) < 3 or values[-1] != 1:
        raise ValueError(f"progress must contain intermediate values and finish at 1: {values}")
    if any(value < 0 or value > 1 for value in values):
        raise ValueError(f"progress outside [0, 1]: {values}")
    if any(right < left for left, right in zip(values, values[1:])):
        raise ValueError(f"progress regressed: {values}")


def validate_output(result: dict[str, Any], *, audio_only: bool = False) -> None:
    streams = result.get("streams", [])
    mimes = {stream.get("mime") for stream in streams}
    if result.get("sizeBytes", 0) <= 0 or not streams:
        raise ValueError(f"empty media output: {result}")
    if "audio/mp4a-latm" not in mimes:
        raise ValueError(f"AAC output stream missing: {mimes}")
    if not audio_only and "video/avc" not in mimes:
        raise ValueError(f"AVC output stream missing: {mimes}")
    if audio_only and any(str(mime).startswith("video/") for mime in mimes):
        raise ValueError(f"audio-only output contains video: {mimes}")


def parse_total_pss(meminfo: str) -> int:
    match = re.search(r"TOTAL PSS:\s+(\d+)", meminfo)
    if not match:
        raise ValueError("worker memory report does not contain TOTAL PSS")
    return int(match.group(1))


def fixture_commands(directory: Path) -> dict[str, list[str]]:
    video = ["-f", "lavfi", "-i", "testsrc2=size=160x90:rate=15", "-f", "lavfi", "-i"]
    return {
        "avi": video + ["sine=frequency=600:sample_rate=22050", "-t", "2", "-c:v", "mpeg4", "-q:v", "8", "-c:a", "mp3", "-f", "avi", str(directory / "matrix.avi")],
        "mkv": video + ["sine=frequency=700:sample_rate=22050", "-t", "2", "-c:v", "mpeg4", "-q:v", "8", "-c:a", "mp3", "-f", "matroska", str(directory / "matrix.mkv")],
        "wmv": video + ["sine=frequency=800:sample_rate=22050", "-t", "2", "-c:v", "wmv2", "-c:a", "wmav2", "-f", "asf", str(directory / "matrix.wmv")],
        "wma": ["-f", "lavfi", "-i", "sine=frequency=900:sample_rate=22050", "-t", "2", "-c:a", "wmav2", "-f", "asf", str(directory / "matrix.wma")],
        "ts": video + ["sine=frequency=1000:sample_rate=22050", "-t", "2", "-c:v", "mpeg2video", "-c:a", "aac", "-f", "mpegts", str(directory / "matrix.ts")],
        "long": ["-f", "lavfi", "-i", "testsrc2=size=640x360:rate=30", "-f", "lavfi", "-i", "sine=frequency=500:sample_rate=44100", "-t", "20", "-c:v", "mpeg4", "-q:v", "5", "-c:a", "mp3", "-f", "avi", str(directory / "matrix-long.avi")],
    }


def make_fixtures(directory: Path) -> dict[str, Path]:
    directory.mkdir(parents=True, exist_ok=True)
    commands = fixture_commands(directory)
    for command in commands.values():
        run(["ffmpeg", "-hide_banner", "-loglevel", "error", *command, "-y"])
    return {name: Path(command[-1]) for name, command in commands.items()}


def callback_expression(method: str, args: list[Any], request_id: str) -> str:
    encoded_args = ",".join(json.dumps(value) for value in [*args, request_id])
    return f"""new Promise(resolve => {{
      const progress = [];
      const previous = window.__viewitBridgeDispatch;
      const timer = setTimeout(() => resolve({{ terminal: {{ event: 'timeout' }}, progress }}), 660000);
      window.__viewitBridgeDispatch = message => {{
        const handled = previous?.(message) ?? false;
        if (message.id !== {json.dumps(request_id)}) return handled;
        if (message.event === 'progress') progress.push(message.progress);
        else {{ clearTimeout(timer); window.__viewitBridgeDispatch = previous; resolve({{ terminal: message, progress }}); }}
        return true;
      }};
      AndroidBridge.{method}({encoded_args});
    }})"""


def run_probe(session: AndroidSession, fixture: Path, ext: str, request_id: str) -> dict[str, Any]:
    payload = base64.b64encode(fixture.read_bytes()).decode()
    with session.cdp(timeout=700) as cdp:
        return cdp.evaluate(callback_expression("probeMediaWorker", [payload, ext], request_id))


def install_local_plugin(session: AndroidSession, plugin: Path) -> None:
    remote = "/data/local/tmp/viewit-media-conformance.zip"
    run(session.adb + ["push", str(plugin), remote])
    session.shell("am", "force-stop", session.package, check=False)
    session.shell(
        "am", "start", "-W", "-a", "ai.viewit.app.action.DEBUG_INSTALL_PLUGIN",
        "-n", session.activity, "--es", "path", remote,
    )
    time.sleep(6)
    session.shell("am", "force-stop", session.package, check=False)
    session.start_app()
    time.sleep(4)


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


def main() -> int:
    args = parse_args()
    if not args.confirm_device_reset:
        raise SystemExit("--confirm-device-reset is required; this test installs the app and replaces the FFmpeg slot")
    output = args.output.resolve()
    build_root = (ROOT / "build").resolve()
    if build_root not in output.parents:
        raise SystemExit("--output must be under build/")
    output.mkdir(parents=True, exist_ok=True)
    fixtures = make_fixtures(output / "fixtures")
    session = AndroidSession(args.serial, args.port)
    session.ensure_device()
    if not args.no_install:
        session.install(args.apk)
    session.clear_app()
    install_local_plugin(session, args.plugin)

    report: dict[str, Any] = {"device": session.profile(), "cases": {}}
    for index, ext in enumerate(("avi", "mkv", "wmv", "wma", "ts")):
        case = run_probe(session, fixtures[ext], ext, f"matrix-{index}-{ext}")
        if case["terminal"].get("event") != "complete":
            raise RuntimeError(f"{ext} failed: {case}")
        validate_progress(case["progress"])
        validate_output(case["terminal"]["result"], audio_only=ext == "wma")
        report["cases"][ext] = case

    malformed = output / "fixtures/malformed.avi"
    malformed.write_bytes(b"not media")
    malformed_result = run_probe(session, malformed, "avi", "matrix-malformed")
    if malformed_result["terminal"].get("event") != "error":
        raise RuntimeError(f"malformed input unexpectedly succeeded: {malformed_result}")
    report["cases"]["malformed"] = malformed_result

    remote_long = "/sdcard/Download/viewit-media-worker-long.avi"
    run(session.adb + ["push", str(fixtures["long"]), remote_long])
    session.shell("am", "broadcast", "-a", "android.intent.action.MEDIA_SCANNER_SCAN_FILE", "-d", f"file://{remote_long}")
    time.sleep(2)
    content_uri = f"content://media/external/video/media/{media_id(session, Path(remote_long).name)}"
    session.shell("am", "force-stop", session.package, check=False)
    session.shell("am", "start", "-W", "-a", "android.intent.action.VIEW", "-d", content_uri, "-t", "video/x-msvideo", "-f", "1", "-n", session.activity)
    time.sleep(4)
    with session.cdp(timeout=700) as cdp:
        large = cdp.evaluate(callback_expression("probeMediaUri", [content_uri, "avi", True], "matrix-large-uri"))
    if large["terminal"].get("event") != "complete":
        raise RuntimeError(f"large content URI failed: {large}")
    validate_progress(large["progress"])
    validate_output(large["terminal"]["result"])
    report["largeUri"] = {"uri": content_uri, **large}
    memory = session.shell("dumpsys", "meminfo", f"{session.package}:media_worker", check=False)
    total_pss_kb = parse_total_pss(memory)
    if total_pss_kb > 96 * 1024:
        raise RuntimeError(f"worker memory exceeded 96 MiB PSS: {total_pss_kb} KB")
    report["workerMemory"] = {"totalPssKb": total_pss_kb, "raw": memory}

    main_pid = session.shell("pidof", session.package)
    worker_pid = session.shell("pidof", f"{session.package}:media_worker")
    with session.cdp(timeout=30) as cdp:
        cancelled = cdp.evaluate(callback_expression("probeMediaWorkerCancellation", [5000], "matrix-cancel"))
    if cancelled["terminal"].get("event") != "error" or "cancel" not in cancelled["terminal"].get("error", "").lower():
        raise RuntimeError(f"cancellation did not report a terminal cancellation error: {cancelled}")
    if session.shell("pidof", session.package) != main_pid:
        raise RuntimeError("main process did not survive worker cancellation")
    deadline = time.time() + 10
    while time.time() < deadline:
        current_worker_pid = session.shell("pidof", f"{session.package}:media_worker", check=False)
        if current_worker_pid != worker_pid:
            break
        time.sleep(0.1)
    else:
        raise RuntimeError(f"worker process did not terminate after cancellation: {worker_pid}")
    report["rebind"] = run_probe(session, fixtures["wmv"], "wmv", "matrix-rebind")
    if report["rebind"]["terminal"].get("event") != "complete":
        raise RuntimeError(f"worker did not recover after cancellation: {report['rebind']}")
    rebound_worker_pid = session.shell("pidof", f"{session.package}:media_worker")
    if not worker_pid or not rebound_worker_pid or rebound_worker_pid == worker_pid:
        raise RuntimeError(f"worker process was not replaced after cancellation: {worker_pid} -> {rebound_worker_pid}")
    report["cancellation"] = {
        "result": cancelled,
        "mainPid": main_pid,
        "workerPid": worker_pid,
        "reboundWorkerPid": rebound_worker_pid,
    }

    with session.cdp(timeout=30) as cdp:
        installed = json.loads(cdp.evaluate("AndroidBridge.listPlugins()"))
    ffmpeg = next(plugin for plugin in installed if plugin["id"] == "ffmpeg-transcoder")
    if not any(value.get("state") == "active" for value in ffmpeg.get("providerHealth", {}).values()):
        raise RuntimeError(f"worker success not attributed to provider health: {ffmpeg}")
    report["providerHealth"] = ffmpeg.get("providerHealth")

    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"[media-worker] PASS: {output / 'report.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
