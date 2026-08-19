#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path

from android_device import AndroidSession, write_bytes
from PIL import Image, ImageStat
from visual_diff import compare_images


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT = ROOT / "build/android-visual"
DEFAULT_BASELINES = ROOT / "e2e/android-visual/baselines"
OFFICE_MANIFEST = ROOT / "apps/mobile/plugins/office-universal/plugin.json"
DEFAULT_PLUGIN = (
    ROOT
    / "apps/mobile/plugins/office-universal/build/output"
    / f"office-universal-{json.loads(OFFICE_MANIFEST.read_text())['version']}-arm64-v8a.zip"
)


@dataclass(frozen=True)
class VisualCase:
    name: str
    path: str
    mime: str
    ready: str


DEFAULT_CASES = (
    VisualCase(
        "docx-report",
        "/sdcard/Download/report.docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "document.querySelectorAll('img').length >= 9 && document.body.innerText.includes('INTERNSHIP REPORT')",
    ),
    VisualCase(
        "pptx-deck",
        "/sdcard/Download/deck.pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "document.body.innerText.includes('1 / 17') && document.querySelectorAll('.slide-element').length > 0",
    ),
    VisualCase(
        "odp-deck",
        "/sdcard/Download/deck.odp",
        "application/vnd.oasis.opendocument.presentation",
        "document.body.innerText.includes('1 / 2') && document.body.innerText.includes('Fidelity Slide One')",
    ),
    VisualCase(
        "xlsx-sample",
        "/sdcard/Download/sample.xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "document.querySelectorAll('table').length > 0 && document.body.innerText.includes('MyLinks')",
    ),
    VisualCase(
        "epub-reader",
        "/sdcard/Download/testing_viewit/sample.epub",
        "application/epub+zip",
        "document.querySelector('.epub-fullscreen') && !document.querySelector('.touch-layer') && document.querySelector('.epub-frame')?.contentDocument?.body",
    ),
    VisualCase(
        "mobi-reader",
        "/sdcard/Download/testing_viewit/sample.mobi",
        "application/x-mobipocket-ebook",
        "document.querySelector('.epub-fullscreen') && !document.querySelector('.touch-layer') && document.querySelector('.epub-frame')?.contentDocument?.body?.innerText?.length > 40",
    ),
)

STABILIZE_JS = """(() => {
  document.documentElement.style.colorScheme = 'dark';
  let style = document.getElementById('__viewit_visual_stable');
  if (!style) {
    style = document.createElement('style');
    style.id = '__viewit_visual_stable';
    style.textContent = `
      *, *::before, *::after { animation: none !important; transition: none !important; caret-color: transparent !important; }
      html { scroll-behavior: auto !important; }
    `;
    document.head.appendChild(style);
  }
  window.scrollTo(0, 0);
  document.querySelectorAll('*').forEach(el => { if (el.scrollTop) el.scrollTop = 0; if (el.scrollLeft) el.scrollLeft = 0; });
  return { width: innerWidth, height: innerHeight, dpr: devicePixelRatio, title: document.title, dark: matchMedia('(prefers-color-scheme: dark)').matches };
})()"""

RESOURCE_READY_JS = """(async () => {
  if (document.fonts?.ready) await document.fonts.ready;
  const images = [...document.images];
  await Promise.all(images.map(async image => {
    if (!image.complete) await new Promise(resolve => image.addEventListener('load', resolve, { once: true }));
    if (image.decode) { try { await image.decode(); } catch {} }
  }));
  await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  return {
    images: images.length,
    decodedImages: images.filter(image => image.complete && image.naturalWidth > 0).length,
    fonts: document.fonts?.status || 'unsupported',
    width: document.documentElement.scrollWidth,
    height: document.documentElement.scrollHeight,
  };
})()"""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", default=os.environ.get("ADB_SERIAL") or os.environ.get("ANDROID_SERIAL", ""))
    parser.add_argument("--port", type=int, default=int(os.environ.get("CDP_PORT", "9333")))
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--baselines", type=Path, default=DEFAULT_BASELINES)
    parser.add_argument("--only", nargs="*", help="Only run named cases")
    parser.add_argument("--update-baselines", action="store_true")
    parser.add_argument("--settle", type=float, default=1.0)
    parser.add_argument("--ready-timeout", type=float, default=20.0)
    parser.add_argument("--plugin-zip", type=Path, default=DEFAULT_PLUGIN)
    parser.add_argument("--no-reset", action="store_true", help="Keep existing app/plugin state")
    parser.add_argument(
        "--confirm-device-reset",
        action="store_true",
        help="Acknowledge that ViewIt app data will be cleared and the Office plugin reinstalled",
    )
    parser.add_argument("--stability-ratio", type=float, default=0.0005)
    parser.add_argument("--max-changed-ratio", type=float, default=0.005)
    parser.add_argument("--max-mean-delta", type=float, default=1.0)
    return parser.parse_args()


def slug(value: str) -> str:
    return re.sub(r"[^a-z0-9._-]+", "-", value.lower()).strip("-")


def profile_name(profile: dict[str, str], capture_size: tuple[int, int], dark: bool) -> str:
    size = profile["size"].splitlines()[-1].split(":", 1)[-1].strip().replace("x", "x")
    density = profile["density"].splitlines()[-1].split(":", 1)[-1].strip()
    fingerprint = hashlib.sha256(profile["build_fingerprint"].encode()).hexdigest()[:10]
    return slug(
        f"{profile['model']}-android{profile['android']}-build{fingerprint}-wv{profile['webview_version']}-app{profile['app_version']}-{size}-d{density}-f{profile['font_scale']}-{profile['locale']}-{'dark' if dark else 'light'}-capture{capture_size[0]}x{capture_size[1]}"
    )


def wait_until(client, expression: str, timeout: float) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        if client.evaluate(f"Boolean({expression})"):
            return
        time.sleep(0.25)
    raise TimeoutError(f"visual case did not become ready: {expression}")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def image_metrics(path: Path) -> dict:
    image = Image.open(path).convert("RGB")
    statistics = ImageStat.Stat(image)
    sample = image.resize((128, 128))
    colors = sample.getcolors(128 * 128) or []
    metrics = {
        "width": image.width,
        "height": image.height,
        "mean": [round(value, 3) for value in statistics.mean],
        "stddev": [round(value, 3) for value in statistics.stddev],
        "sample_color_count": len(colors),
    }
    if min(image.size) < 100 or max(statistics.stddev) < 3 or len(colors) < 8:
        raise ValueError(f"capture appears blank or invalid: {metrics}")
    return metrics


def main() -> int:
    args = parse_args()
    if args.update_baselines and args.only:
        print("--update-baselines cannot be combined with --only", file=sys.stderr)
        return 2
    if not args.no_reset and not args.confirm_device_reset:
        print(
            "visual tests clear ViewIt app data and reinstall office-universal; pass --confirm-device-reset or --no-reset",
            file=sys.stderr,
        )
        return 2
    selected = [case for case in DEFAULT_CASES if not args.only or case.name in args.only]
    if args.only:
        unknown = sorted(set(args.only) - {case.name for case in DEFAULT_CASES})
        if unknown:
            print(f"unknown visual case(s): {', '.join(unknown)}", file=sys.stderr)
            return 2

    session = AndroidSession(args.serial, args.port)
    session.ensure_device()
    if not args.no_reset:
        session.clear_app()
        session.start_app()
        time.sleep(1)
        if not args.plugin_zip.is_file():
            print(f"missing Office plugin ZIP: {args.plugin_zip}", file=sys.stderr)
            return 2
        remote_plugin = f"/sdcard/Download/{args.plugin_zip.name}"
        session.shell("mkdir", "-p", "/sdcard/Download")
        from android_device import run

        run(session.adb + ["push", str(args.plugin_zip), remote_plugin])
        with session.cdp() as client:
            installed = client.evaluate(
                f"(() => {{ window.AndroidBridge.installPluginLocal({json.dumps(remote_plugin)}, 'visual_office'); return true; }})()"
            )
        if not installed:
            print("failed to request Office plugin installation", file=sys.stderr)
            return 2
        time.sleep(3)
        with session.cdp() as client:
            plugins = client.evaluate(
                "JSON.parse(window.AndroidBridge.listPlugins()).map(plugin => ({ id: plugin.id, version: plugin.version }))"
            )
        office_plugins = [plugin for plugin in plugins if plugin["id"] == "office-universal"]
        if len(office_plugins) != 1:
            print(f"unexpected installed plugin state: {plugins}", file=sys.stderr)
            return 2
    else:
        with session.cdp() as client:
            plugins = client.evaluate(
                "JSON.parse(window.AndroidBridge.listPlugins()).map(plugin => ({ id: plugin.id, version: plugin.version }))"
            )
    session.grant_file_access()
    profile = session.profile()
    fixture_hashes = {case.name: session.file_sha256(case.path) for case in selected}
    output = args.output.resolve()
    if output.exists():
        shutil.rmtree(output)
    (output / "candidates").mkdir(parents=True)
    (output / "stability").mkdir()
    (output / "diffs").mkdir()

    results = []
    failures = 0
    profile_slug = "unknown"
    for case in selected:
        print(f"[visual] {case.name}: open {case.path}")
        if "No such file" in session.shell("ls", "-l", case.path, check=False):
            print(f"[visual] FAIL missing fixture: {case.path}")
            failures += 1
            results.append({"case": case.name, "status": "missing-fixture", "path": case.path})
            continue
        try:
            session.open_file(case.path, case.mime)
            time.sleep(args.settle)
            with session.cdp() as client:
                client.call("Page.enable")
                wait_until(client, case.ready, args.ready_timeout)
                resources = client.evaluate(RESOURCE_READY_JS)
                if resources["decodedImages"] != resources["images"]:
                    raise RuntimeError(f"not all images decoded: {resources}")
                viewport = client.evaluate(STABILIZE_JS)
                time.sleep(0.25)
                first = output / "candidates" / f"{case.name}.png"
                second = output / "stability" / f"{case.name}.png"
                write_bytes(first, client.screenshot())
                time.sleep(0.35)
                client.evaluate(STABILIZE_JS)
                write_bytes(second, client.screenshot())

            candidate_metrics = image_metrics(first)
            profile_slug = profile_name(
                profile,
                (candidate_metrics["width"], candidate_metrics["height"]),
                bool(viewport["dark"]),
            )
            baseline = args.baselines / profile_slug / f"{case.name}.png"
            stability = compare_images(
                first,
                second,
                output / "diffs" / f"{case.name}.stability.png",
                max_changed_ratio=args.stability_ratio,
                max_mean_delta=0.25,
            )
            result = {
                "case": case.name,
                "fixture": case.path,
                "candidate": str(first),
                "candidate_sha256": sha256(first),
                "baseline": str(baseline),
                "stability": stability.to_dict(),
                "viewport": viewport,
                "resources": resources,
                "image": candidate_metrics,
                "fixture_sha256": fixture_hashes[case.name],
            }
            if not stability.passed:
                result["status"] = "unstable"
                failures += 1
            elif args.update_baselines:
                result["status"] = "baseline-updated"
            elif not baseline.exists():
                result["status"] = "missing-baseline"
                failures += 1
            else:
                visual = compare_images(
                    baseline,
                    first,
                    output / "diffs" / f"{case.name}.png",
                    max_changed_ratio=args.max_changed_ratio,
                    max_mean_delta=args.max_mean_delta,
                )
                result["visual"] = visual.to_dict()
                result["status"] = "passed" if visual.passed else "changed"
                failures += 0 if visual.passed else 1
            print(f"[visual] {result['status']} {case.name} stability={stability.changed_ratio:.6f}")
            results.append(result)
        except Exception as error:
            failures += 1
            results.append({"case": case.name, "status": "error", "error": str(error)})
            print(f"[visual] FAIL {case.name}: {error}")

    report = {
        "profile": profile,
        "profile_slug": profile_slug,
        "baselines": str(args.baselines),
        "output": str(output),
        "update_baselines": args.update_baselines,
        "plugins": plugins,
        "failures": failures,
        "results": results,
    }
    if not args.update_baselines and failures == 0:
        profile_path = args.baselines / profile_slug / "profile.json"
        if not profile_path.exists():
            failures += 1
            report["failures"] = failures
            report["profile_error"] = f"missing baseline profile: {profile_path}"
        else:
            expected_profile = json.loads(profile_path.read_text())
            expected_fixtures = expected_profile.get("fixtures", {})
            fixture_mismatch = any(
                expected_fixtures.get(case_name) != digest
                for case_name, digest in fixture_hashes.items()
            )
            if fixture_mismatch:
                failures += 1
                report["failures"] = failures
                report["profile_error"] = "fixture hashes differ from baseline profile"
            elif expected_profile.get("plugins") != plugins:
                failures += 1
                report["failures"] = failures
                report["profile_error"] = "installed plugin state differs from baseline profile"
    if args.update_baselines and failures == 0:
        target = args.baselines / profile_slug
        staged = args.baselines / f".{profile_slug}.staged"
        previous = args.baselines / f".{profile_slug}.previous"
        for temporary in (staged, previous):
            if temporary.exists():
                shutil.rmtree(temporary)
        staged.mkdir(parents=True)
        for result in results:
            shutil.copy2(result["candidate"], staged / f"{result['case']}.png")
        profile_document = {
            **profile,
            "profile_slug": profile_slug,
            "capture_size": [results[0]["image"]["width"], results[0]["image"]["height"]],
            "fixtures": fixture_hashes,
            "plugins": plugins,
            "cases": [result["case"] for result in results],
        }
        (staged / "profile.json").write_text(json.dumps(profile_document, indent=2) + "\n")
        target.parent.mkdir(parents=True, exist_ok=True)
        try:
            if target.exists():
                target.rename(previous)
            staged.rename(target)
        except Exception:
            if not target.exists() and previous.exists():
                previous.rename(target)
            raise
        finally:
            if staged.exists():
                shutil.rmtree(staged)
        if previous.exists():
            shutil.rmtree(previous)
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"[visual] report: {output / 'report.json'}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
