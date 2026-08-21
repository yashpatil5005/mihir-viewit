#!/usr/bin/env python3
"""Validate the packaged Linux desktop UI using native X11 window captures."""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageStat


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_BINARY = Path(os.environ.get("VIEWIT_DESKTOP_BINARY", "/mnt/devspace/rust-targets/release/viewit-desktop"))
DEFAULT_FIXTURES = ROOT / "build" / "desktop-fixtures"
DEFAULT_OUTPUT = ROOT / "build" / "desktop-offline-visual"


@dataclass(frozen=True)
class VisualCase:
    file: str
    expected: tuple[str, ...]


CASES = (
    VisualCase("sample.txt", ("hello", "viewit offline desktop")),
    VisualCase("sample.md", ("viewit offline markdown", "item 1")),
    VisualCase("sample.json", ("viewit", "offline", "42")),
    VisualCase("sample.csv", ("alice", "developer")),
    VisualCase("sample.docx", ("hello from offline", "document")),
    VisualCase("sample.xlsx", ("desktop product", "viewit native")),
    VisualCase("sample.pptx", ("offline desktop slide",)),
    VisualCase("sample.odt", ("offline opendocument",)),
    VisualCase("sample.ods", ("ods cell",)),
    VisualCase("sample.odp", ("odp slide",)),
    VisualCase("sample.epub", ("chapter 1", "offline desktop")),
    VisualCase("sample.pdf", ("sample.pdf", "hide thumbnails")),
    VisualCase("sample.zip", ("archive", "hello")),
    VisualCase("sample.wav", ("sample.wav", "volume", "speed")),
)


def run(args: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(args, check=True, text=True, **kwargs)


def require_tools() -> None:
    missing = [tool for tool in ("xwininfo", "xprop", "xwd", "ffmpeg", "tesseract") if not shutil.which(tool)]
    if missing:
        raise SystemExit(f"missing desktop visual test tools: {', '.join(missing)}")
    if not os.environ.get("DISPLAY"):
        raise SystemExit("DISPLAY is not set; run this test in an X11/XWayland desktop session")


def client_windows() -> list[str]:
    output = run(["xprop", "-root", "_NET_CLIENT_LIST"], capture_output=True).stdout
    return re.findall(r"0x[0-9a-f]+", output, re.IGNORECASE)


def window_pid(window_id: str) -> int | None:
    result = subprocess.run(
        ["xprop", "-id", window_id, "_NET_WM_PID"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    output = result.stdout
    match = re.search(r"=\s*(\d+)", output)
    return int(match.group(1)) if match else None


def window_title(window_id: str) -> str:
    result = subprocess.run(
        ["xprop", "-id", window_id, "_NET_WM_NAME"],
        capture_output=True,
        text=True,
        check=False,
    )
    match = re.search(r'=\s*"(.*)"', result.stdout)
    return match.group(1) if match else ""


def wait_for_window(pid: int, timeout: float) -> str:
    deadline = time.time() + timeout
    while time.time() < deadline:
        for window_id in client_windows():
            if window_pid(window_id) == pid:
                return window_id
        time.sleep(0.2)
    raise TimeoutError(f"native window did not appear for pid {pid}")


def capture(window_id: str, destination: Path) -> None:
    xwd_path = destination.with_suffix(".xwd")
    run(["xwd", "-silent", "-id", window_id, "-out", str(xwd_path)])
    run(
        [
            "ffmpeg",
            "-loglevel",
            "error",
            "-y",
            "-i",
            str(xwd_path),
            "-frames:v",
            "1",
            "-update",
            "1",
            str(destination),
        ]
    )
    xwd_path.unlink(missing_ok=True)


def ocr(image: Path) -> str:
    return run(["tesseract", str(image), "stdout"], capture_output=True).stdout.strip()


def image_metrics(image: Path) -> dict[str, object]:
    rendered = Image.open(image).convert("RGB")
    stats = ImageStat.Stat(rendered)
    colors = rendered.resize((128, 128)).getcolors(128 * 128) or []
    return {
        "width": rendered.width,
        "height": rendered.height,
        "mean": [round(value, 3) for value in stats.mean],
        "stddev": [round(value, 3) for value in stats.stddev],
        "sample_colors": len(colors),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--fixtures", type=Path, default=DEFAULT_FIXTURES)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--settle", type=float, default=5.0)
    parser.add_argument("--only", nargs="*")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    require_tools()
    if not args.binary.is_file() or not os.access(args.binary, os.X_OK):
        raise SystemExit(f"desktop ELF is missing or not executable: {args.binary}")

    args.output.mkdir(parents=True, exist_ok=True)
    selected = [case for case in CASES if not args.only or case.file in args.only]
    results = []

    for case in selected:
        fixture = args.fixtures / case.file
        if not fixture.is_file():
            results.append({"file": case.file, "status": "missing"})
            print(f"[desktop-visual] FAIL {case.file}: fixture missing")
            continue

        stdout = (args.output / case.file).with_suffix(".stdout.log").open("w")
        stderr = (args.output / case.file).with_suffix(".stderr.log").open("w")
        diagnostic_path = args.output / f"{case.file}.dom.json"
        diagnostic_path.unlink(missing_ok=True)
        env = {**os.environ, "VIEWIT_DESKTOP_E2E_REPORT": str(diagnostic_path)}
        process = subprocess.Popen(
            [str(args.binary), str(fixture.resolve())],
            stdout=stdout,
            stderr=stderr,
            env=env,
        )
        try:
            window_id = wait_for_window(process.pid, 15)
            time.sleep(args.settle)
            image = args.output / f"{case.file}.png"
            capture(window_id, image)
            text = ocr(image)
            metrics = image_metrics(image)
            diagnostic = json.loads(diagnostic_path.read_text()) if diagnostic_path.exists() else {}
            diagnostic_body = str(diagnostic.get("body", ""))
            normalized = " ".join(f"{text}\n{diagnostic_body}".lower().split())
            missing = [token for token in case.expected if token not in normalized]
            errors = ("could not open", "unknown document kind", "failed to", "error:")
            has_error = bool(diagnostic.get("error")) or any(token in normalized for token in errors)
            nonblank = max(metrics["stddev"]) >= 3 and metrics["sample_colors"] >= 8
            passed = nonblank and not has_error and not missing
            result = {
                "file": case.file,
                "status": "passed" if passed else "failed",
                "expected": case.expected,
                "missing": missing,
                "has_error": has_error,
                "ocr": text,
                "diagnostic": diagnostic,
                "image": str(image),
                "metrics": metrics,
            }
            results.append(result)
            print(
                f"[desktop-visual] {'PASS' if passed else 'FAIL'} {case.file} "
                f"colors={metrics['sample_colors']} missing={missing}"
            )
        except Exception as error:
            results.append({"file": case.file, "status": "error", "error": str(error)})
            print(f"[desktop-visual] FAIL {case.file}: {error}")
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
            stdout.close()
            stderr.close()

    failures = [result for result in results if result["status"] != "passed"]
    report = {"binary": str(args.binary), "fixtures": str(args.fixtures), "results": results, "failures": len(failures)}
    (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"[desktop-visual] {len(results) - len(failures)}/{len(results)} passed")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
