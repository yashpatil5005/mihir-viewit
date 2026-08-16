#!/usr/bin/env python3
"""Physical-device regression test for Android plugin lifecycle behavior.

This test installs an APK and clears ViewIt app data. It requires an explicit
--confirm-device-reset acknowledgement and never mutates catalog artifacts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
import time
import urllib.request
import zipfile
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from android_device import AndroidSession, run


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_APK = ROOT / "dist/viewit-android-arm64-release.apk"
DEFAULT_NATIVE_CURRENT = (
    ROOT / "apps/mobile/plugins/office-universal/build/output/office-universal-0.1.4-arm64-v8a.zip"
)
DEFAULT_NATIVE_UPGRADE = (
    ROOT / "apps/mobile/plugins/office-universal/build/output/office-universal-0.1.5-arm64-v8a.zip"
)
DEFAULT_JS_PLUGIN = ROOT / "plugins/editor-base-0.1.0.zip"
DEFAULT_DOCUMENT = "/sdcard/Download/report.docx"
DEFAULT_OUTPUT = ROOT / "build/android-plugin-lifecycle"
PINNED_ARTIFACTS = {
    DEFAULT_NATIVE_CURRENT: (
        "https://omnia.mihirpatil.co/plugins/office-universal-0.1.4-arm64-v8a.zip",
        "83d57589537b8762199ad75be78fac273bbe50a482c155beeee12843143f2fc2",
    ),
    DEFAULT_NATIVE_UPGRADE: (
        "https://omnia.mihirpatil.co/plugins/office-universal-0.1.5-arm64-v8a.zip",
        "f40de414be5e65a9ca2bc183315df4fbe72229c243015fb39bb46ed03034e098",
    ),
}


@dataclass
class Check:
    name: str
    passed: bool
    expected: Any
    actual: Any


class LifecycleFailure(RuntimeError):
    pass


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", default=os.environ.get("ADB_SERIAL") or os.environ.get("ANDROID_SERIAL", ""))
    parser.add_argument("--port", type=int, default=int(os.environ.get("CDP_PORT", "9344")))
    parser.add_argument("--apk", type=Path, default=DEFAULT_APK)
    parser.add_argument("--native-current", type=Path, default=DEFAULT_NATIVE_CURRENT)
    parser.add_argument("--native-upgrade", type=Path, default=DEFAULT_NATIVE_UPGRADE)
    parser.add_argument("--js-plugin", type=Path, default=DEFAULT_JS_PLUGIN)
    parser.add_argument("--document", default=DEFAULT_DOCUMENT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--no-install", action="store_true", help="Use the currently installed APK")
    parser.add_argument(
        "--confirm-device-reset",
        action="store_true",
        help="Acknowledge that ViewIt will be installed and all of its app data cleared",
    )
    return parser.parse_args()


def read_manifest(path: Path) -> dict[str, Any]:
    with zipfile.ZipFile(path) as archive:
        return json.loads(archive.read("plugin.json"))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def ensure_pinned_artifact(path: Path) -> None:
    pinned = PINNED_ARTIFACTS.get(path)
    if path.is_file():
        if pinned is not None and sha256(path) != pinned[1]:
            raise LifecycleFailure(f"pinned fixture checksum mismatch: {path.name}: {sha256(path)}")
        return
    if pinned is None:
        return
    url, expected = pinned
    cache = ROOT / "build/plugin-lifecycle-artifacts" / path.name
    cache.parent.mkdir(parents=True, exist_ok=True)
    print(f"[lifecycle] download pinned fixture {url}")
    urllib.request.urlretrieve(url, cache)
    actual = sha256(cache)
    if actual != expected:
        cache.unlink(missing_ok=True)
        raise LifecycleFailure(f"pinned fixture checksum mismatch: {path.name}: {actual}")
    path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(cache, path)


def archive_expanded_size(path: Path) -> int:
    with zipfile.ZipFile(path) as archive:
        return sum(entry.file_size for entry in archive.infolist())


def validate_output_path(path: Path) -> Path:
    output = path.resolve()
    build_root = (ROOT / "build").resolve()
    if output == build_root or build_root not in output.parents:
        raise LifecycleFailure(f"--output must be a child of {build_root}")
    return output


def find_apkanalyzer() -> str:
    found = shutil.which("apkanalyzer")
    if found:
        return found
    sdk = Path(os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT") or Path.home() / "Android/Sdk")
    candidate = sdk / "cmdline-tools/latest/bin/apkanalyzer"
    if candidate.is_file():
        return str(candidate)
    raise LifecycleFailure("apkanalyzer is required to verify the APK package")


def apk_identity(path: Path) -> dict[str, str]:
    analyzer = find_apkanalyzer()
    package = run([analyzer, "manifest", "application-id", str(path)]).strip()
    version_code = run([analyzer, "manifest", "version-code", str(path)]).strip()
    version_name = run([analyzer, "manifest", "version-name", str(path)]).strip()
    return {"package": package, "versionCode": version_code, "versionName": version_name}


def source_identity() -> dict[str, Any]:
    revision = run(["git", "-C", str(ROOT), "rev-parse", "HEAD"]).strip()
    status = run(["git", "-C", str(ROOT), "status", "--porcelain=v1", "--untracked-files=all"])
    diff = run(["git", "-C", str(ROOT), "diff", "--no-ext-diff", "--binary", "HEAD"])
    untracked = []
    for line in status.splitlines():
        if line.startswith("?? "):
            path = ROOT / line[3:]
            if path.is_file():
                untracked.append({"path": line[3:], "sha256": sha256(path)})
    fingerprint_body = json.dumps(
        {"revision": revision, "status": status, "diff": diff, "untracked": untracked},
        sort_keys=True,
    ).encode()
    return {
        "revision": revision,
        "dirty": bool(status.strip()),
        "workspaceFingerprint": hashlib.sha256(fingerprint_body).hexdigest(),
        "untracked": untracked,
    }


def write_adversarial_plugins(directory: Path) -> dict[str, Path]:
    directory.mkdir(parents=True, exist_ok=True)
    unsafe_id = directory / "unsafe-id.zip"
    traversal = directory / "zip-traversal.zip"
    checksum = directory / "checksum-mismatch.zip"
    base_manifest = {
        "name": "Lifecycle Negative Fixture",
        "version": "1.0.0",
        "description": "Generated by android-plugin-lifecycle.py",
        "minAppVersion": 1,
        "entryClass": "",
        "supportedFormats": ["txt"],
        "downloadUrl": "",
        "sizeBytes": 1,
        "installedSizeBytes": 1024,
        "checksum": "",
        "abi": "",
        "abiVersion": 1,
        "runtime": "js",
        "jsEntry": "index.js",
    }
    with zipfile.ZipFile(unsafe_id, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("plugin.json", json.dumps({**base_manifest, "id": "../unsafe-plugin"}))
        archive.writestr("index.js", "export default {};")
    with zipfile.ZipFile(traversal, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("plugin.json", json.dumps({**base_manifest, "id": "lifecycle-traversal"}))
        archive.writestr("index.js", "export default {};")
        archive.writestr("../escaped.txt", "must not be extracted")
    with zipfile.ZipFile(checksum, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr(
            "plugin.json",
            json.dumps({**base_manifest, "id": "lifecycle-checksum", "checksum": "0" * 64}),
        )
        archive.writestr("index.js", "export default {};")
    return {"unsafe-id": unsafe_id, "zip-traversal": traversal, "checksum-mismatch": checksum}


def install_expression(remote_path: str, callback_id: str, timeout_ms: int = 90_000) -> str:
    return f"""new Promise(resolve => {{
      const id = {json.dumps(callback_id)};
      const events = [];
      const previous = window.__viewitBridgeDispatch;
      const finish = terminal => {{
        clearTimeout(timer);
        window.__viewitBridgeDispatch = previous;
        resolve({{ terminal, events, callbackRestored: window.__viewitBridgeDispatch === previous }});
      }};
      const timer = setTimeout(() => finish({{ event: 'timeout', error: 'bridge callback timeout' }}), {timeout_ms});
      window.__viewitBridgeDispatch = event => {{
        const handled = previous?.(event) ?? false;
        if (event.id !== id) return handled;
        const copy = JSON.parse(JSON.stringify(event));
        events.push(copy);
        if (copy.event === 'complete' || copy.event === 'error' || copy.event === 'restart-required') finish(copy);
        return true;
      }};
      try {{
        window.AndroidBridge.installPluginLocal({json.dumps(remote_path)}, id);
      }} catch (error) {{
        finish({{ event: 'error', error: String(error?.message || error) }});
      }}
    }})"""


def render_expression(document_uri: str, timeout_ms: int = 60_000) -> str:
    return f"""new Promise(resolve => {{
      const id = 'render_' + Date.now() + '_' + Math.random().toString(36).slice(2);
       const previous = window.__viewitBridgeDispatch;
      const finish = payload => {{
        clearTimeout(timer);
         window.__viewitBridgeDispatch = previous;
         resolve({{ payload, callbacksRestored: window.__viewitBridgeDispatch === previous }});
      }};
      const timer = setTimeout(() => finish({{ id, error: 'document callback timeout' }}), {timeout_ms});
       window.__viewitBridgeDispatch = payload => {{
         const handled = previous?.(payload) ?? false;
         if (payload.id === id) finish(JSON.parse(JSON.stringify(payload)));
         return payload.id === id || handled;
       }};
      window.AndroidBridge.renderDocumentWithPlugin('office-universal', {json.dumps(document_uri)}, 'docx', id);
    }})"""


def main() -> int:
    args = parse_args()
    if not args.confirm_device_reset:
        print("plugin lifecycle tests clear ViewIt app data; pass --confirm-device-reset", file=sys.stderr)
        return 2

    try:
        ensure_pinned_artifact(args.native_current)
        ensure_pinned_artifact(args.native_upgrade)
        output = validate_output_path(args.output)
    except Exception as error:
        print(str(error), file=sys.stderr)
        return 2

    required = [args.native_current, args.native_upgrade, args.js_plugin]
    if not args.no_install:
        required.append(args.apk)
    missing = [str(path) for path in required if not path.is_file()]
    if missing:
        print(f"missing required artifact(s): {', '.join(missing)}", file=sys.stderr)
        return 2

    current_manifest = read_manifest(args.native_current)
    upgrade_manifest = read_manifest(args.native_upgrade)
    js_manifest = read_manifest(args.js_plugin)
    if current_manifest["id"] != upgrade_manifest["id"] or current_manifest["version"] == upgrade_manifest["version"]:
        print("native artifacts must be different versions of the same plugin", file=sys.stderr)
        return 2
    if (
        current_manifest["id"] != "office-universal"
        or upgrade_manifest["id"] != "office-universal"
        or current_manifest["version"] != "0.1.4"
        or upgrade_manifest["version"] != "0.1.5"
        or "docx" not in current_manifest["supportedFormats"]
        or "docx" not in upgrade_manifest["supportedFormats"]
    ):
        print("native lifecycle fixtures must be office-universal 0.1.4/0.1.5 packages with DOCX support", file=sys.stderr)
        return 2
    expanded_size = archive_expanded_size(args.native_current)
    if expanded_size <= int(current_manifest.get("installedSizeBytes", 0)):
        print("current native fixture must exceed its advisory installedSizeBytes", file=sys.stderr)
        return 2

    if output.exists():
        shutil.rmtree(output)
    fixtures = write_adversarial_plugins(output / "fixtures")
    checks: list[Check] = []
    report: dict[str, Any] = {
        "status": "running",
        "source": source_identity(),
        "artifacts": {
            "apk": None if args.no_install else {"path": str(args.apk.resolve()), "sha256": sha256(args.apk)},
            "nativeCurrent": {"manifest": current_manifest, "sha256": sha256(args.native_current), "expandedSize": expanded_size},
            "nativeUpgrade": {"manifest": upgrade_manifest, "sha256": sha256(args.native_upgrade)},
            "jsPlugin": {"manifest": js_manifest, "sha256": sha256(args.js_plugin)},
            "negativeFixtures": {
                name: {"sha256": sha256(path), "size": path.stat().st_size}
                for name, path in fixtures.items()
            },
        },
        "checks": [],
    }

    def check(name: str, condition: bool, expected: Any, actual: Any) -> None:
        item = Check(name, bool(condition), expected, actual)
        checks.append(item)
        print(f"[lifecycle] {'PASS' if item.passed else 'FAIL'} {name}")
        if not item.passed:
            raise LifecycleFailure(f"{name}: expected {expected!r}, got {actual!r}")

    session = AndroidSession(args.serial, args.port)
    remote_dir = "/sdcard/Download/viewit-plugin-lifecycle"
    try:
        session.ensure_device()
        if not args.no_install:
            identity = apk_identity(args.apk.resolve())
            check("APK package is ViewIt", identity["package"] == session.package, session.package, identity)
            session.install(args.apk.resolve())
        session.clear_app()
        session.start_app()
        session.grant_file_access()
        session.forward_webview()
        report["device"] = session.profile()
        installed_dump = session.shell("dumpsys", "package", session.package)
        installed_version = re.search(r"versionCode=(\d+)", installed_dump)
        report["installedApp"] = {
            "package": session.package,
            "versionCode": installed_version.group(1) if installed_version else "unknown",
            "versionName": report["device"]["app_version"],
        }
        if not args.no_install:
            check(
                "installed version matches APK",
                report["installedApp"]["versionCode"] == identity["versionCode"]
                and report["installedApp"]["versionName"] == identity["versionName"]
                and report["installedApp"]["package"] == identity["package"],
                identity,
                report["installedApp"],
            )
        document_size = session.shell("stat", "-c", "%s", args.document, check=False)
        document_exists = document_size.isdigit() and int(document_size) > 0
        check("document fixture exists", document_exists, "non-empty file", document_size)
        report["document"] = {
            "path": args.document,
            "size": int(document_size),
            "sha256": session.file_sha256(args.document),
        }

        session.shell("rm", "-rf", remote_dir, check=False)
        session.shell("mkdir", "-p", remote_dir)
        local_artifacts = [args.native_current, args.native_upgrade, args.js_plugin, *fixtures.values()]
        remote_paths: dict[str, str] = {}
        for path in local_artifacts:
            remote = f"{remote_dir}/{path.name}"
            run(session.adb + ["push", str(path.resolve()), remote])
            remote_paths[str(path.resolve())] = remote

        def evaluate(expression: str, timeout: int = 100) -> Any:
            with session.cdp(timeout=timeout) as client:
                return client.evaluate(expression)

        def plugins() -> list[dict[str, Any]]:
            return evaluate("JSON.parse(window.AndroidBridge.listPlugins())")

        check("clean start has no plugins", plugins() == [], [], plugins())

        initial = evaluate(install_expression(remote_paths[str(args.native_current.resolve())], "native_initial"))
        check("initial native install completes", initial["terminal"].get("event") == "complete" and initial["callbackRestored"], "complete with restored callback", initial)
        installed = plugins()
        check(
            "initial native version is active",
            any(p["id"] == current_manifest["id"] and p["version"] == current_manifest["version"] for p in installed),
            current_manifest["version"],
            installed,
        )

        rendered = evaluate(render_expression(f"file://{args.document}"), timeout=70)
        payload = rendered["payload"]
        check(
            "unified bridge dispatcher receives rendered document",
            rendered["callbacksRestored"] and "document" in payload and "error" not in payload,
            "document payload with restored callback",
            payload.get("error") or {"keys": list(payload.keys()), "callbacksRestored": rendered["callbacksRestored"]},
        )

        removed = evaluate("window.AndroidBridge.removePlugin('office-universal')")
        check("native plugin removal succeeds", removed is True, True, removed)
        check("removed plugin leaves active inventory", not any(p["id"] == "office-universal" for p in plugins()), [], plugins())

        warm = evaluate(install_expression(remote_paths[str(args.native_current.resolve())], "native_warm"))
        check("same native artifact warm-reinstalls", warm["terminal"].get("event") == "complete" and warm["callbackRestored"], "complete with restored callback", warm)
        rendered = evaluate(render_expression(f"file://{args.document}"), timeout=70)
        payload = rendered["payload"]
        check(
            "warm-reinstalled native plugin renders",
            rendered["callbacksRestored"] and "document" in payload and "error" not in payload,
            "document payload with restored callbacks",
            payload.get("error") or list(payload.keys()),
        )

        upgrade = evaluate(install_expression(remote_paths[str(args.native_upgrade.resolve())], "native_upgrade"))
        upgrade_error = str(upgrade["terminal"].get("error", ""))
        check(
            "native upgrade requests restart",
            upgrade["terminal"].get("event") == "error" and "Restart" in upgrade_error and upgrade["callbackRestored"],
            "restart error with restored callback",
            upgrade,
        )
        active_before_restart = plugins()
        check(
            "old native version stays active before restart",
            any(p["id"] == current_manifest["id"] and p["version"] == current_manifest["version"] for p in active_before_restart),
            current_manifest["version"],
            active_before_restart,
        )
        rendered = evaluate(render_expression(f"file://{args.document}"), timeout=70)
        payload = rendered["payload"]
        check(
            "old native instance renders after upgrade staging",
            rendered["callbacksRestored"] and "document" in payload and "error" not in payload,
            "document payload with restored callbacks",
            payload.get("error") or list(payload.keys()),
        )

        session.shell("am", "force-stop", session.package)
        session.start_app()
        time.sleep(0.5)
        session.forward_webview()
        active_after_restart = plugins()
        check(
            "staged native upgrade activates after cold restart",
            any(p["id"] == upgrade_manifest["id"] and p["version"] == upgrade_manifest["version"] for p in active_after_restart),
            upgrade_manifest["version"],
            active_after_restart,
        )

        rendered = evaluate(render_expression(f"file://{args.document}"), timeout=70)
        payload = rendered["payload"]
        check(
            "upgraded native plugin renders after restart",
            rendered["callbacksRestored"] and "document" in payload and "error" not in payload,
            "document payload with restored callbacks",
            payload.get("error") or list(payload.keys()),
        )

        js_install = evaluate(install_expression(remote_paths[str(args.js_plugin.resolve())], "js_install"))
        check(
            "JavaScript plugin installs",
            js_install["terminal"].get("event") == "complete" and js_install["callbackRestored"],
            "complete with restored callback",
            js_install,
        )
        bundle = evaluate(f"window.AndroidBridge.loadPluginBundle({json.dumps(js_manifest['id'])})")
        check("JavaScript plugin bundle loads", isinstance(bundle, str) and bundle.startswith("b64gz:"), "b64gz bundle", bundle[:32] if bundle else bundle)

        expected_errors = {
            "unsafe-id": ("id contains invalid characters",),
            "zip-traversal": ("unsafe path", "zip entry path"),
            "checksum-mismatch": ("Checksum mismatch",),
        }
        for fixture_name, path in fixtures.items():
            inventory_before = plugins()
            rejected = evaluate(install_expression(remote_paths[str(path.resolve())], f"negative_{fixture_name}"))
            error = str(rejected["terminal"].get("error", ""))
            check(
                f"{fixture_name} package is rejected",
                rejected["terminal"].get("event") == "error"
                and rejected["callbackRestored"]
                and any(fragment.lower() in error.lower() for fragment in expected_errors[fixture_name]),
                f"{' or '.join(expected_errors[fixture_name])} with restored callback",
                rejected,
            )
            check(
                f"{fixture_name} rejection preserves inventory",
                plugins() == inventory_before,
                inventory_before,
                plugins(),
            )
        report["status"] = "passed"
    except Exception as error:
        report["status"] = "failed"
        report["error"] = str(error)
    finally:
        try:
            session.shell("rm", "-rf", remote_dir, check=False)
        except Exception:
            pass
        shutil.rmtree(output / "fixtures", ignore_errors=True)
        report["checks"] = [asdict(item) for item in checks]
        report["summary"] = {
            "passed": sum(item.passed for item in checks),
            "failed": sum(not item.passed for item in checks),
        }
        output.mkdir(parents=True, exist_ok=True)
        (output / "results.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        print(f"[lifecycle] evidence: {output / 'results.json'}")

    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
