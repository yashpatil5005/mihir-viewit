#!/usr/bin/env python3
"""Reconcile plugins/catalog.json against staged plugin zips + manifests.

Idempotent: every run rebuilds the office-ooxml (per-abi) and pptx-vanilla
(runtim=js) catalog entries from the on-disk zips and .json manifests, then
leaves other entries (e.g. ffmpeg-transcoder, whose big zips are only deployed,
not staged locally) untouched.

Usage:
  python3 scripts/update-catalog.py            # rebuild in place
  python3 scripts/update-catalog.py --check    # fail if catalog would change
"""
import argparse
import hashlib
import json
import zipfile
from pathlib import Path

ROOT = Path(__file__).parent.parent
CATALOG = ROOT / "plugins" / "catalog.json"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def installed_size(zip_path: Path) -> int:
    with zipfile.ZipFile(zip_path) as z:
        return sum(i.file_size for i in z.infolist())


def office_ooxml_entries() -> list[dict]:
    manifest = json.loads((ROOT / "plugins" / "office-ooxml" / "plugin.json").read_text())
    base = {
        "id": manifest["id"],
        "name": manifest["name"],
        "version": manifest["version"],
        "description": manifest["description"],
        "minAppVersion": manifest.get("minAppVersion", 1),
        "entryClass": manifest["entryClass"],
        "capabilities": manifest.get("capabilities", []),
        "supportedFormats": manifest.get("supportedFormats", []),
        "runtime": manifest.get("runtime", "android-dex-rust-sidecar"),
        "abiVersion": manifest.get("abiVersion", 1),
        "installedSizeBytes": int(manifest.get("installedSizeBytes", 0)),
    }
    entries = []
    for abi in ("arm64-v8a", "x86_64"):
        zip_path = ROOT / "plugins" / f"office-ooxml-{manifest['version']}-{abi}.zip"
        if not zip_path.exists():
            continue
        entries.append({
            **base,
            "abi": abi,
            "sizeBytes": zip_path.stat().st_size,
            "installedSizeBytes": installed_size(zip_path),
            "checksum": sha256(zip_path),
            "downloadUrl": (
                f"https://github.com/mihir0209/ViewIt/releases/latest/download/"
                f"office-ooxml-{manifest['version']}-{abi}.zip"
            ),
        })
    return entries


def pptx_vanilla_entries() -> list[dict]:
    manifest = json.loads((ROOT / "apps" / "mobile" / "plugins" / "pptx-vanilla" / "plugin.json").read_text())
    plugin_dir = ROOT / "dist" / "plugins"
    zip_paths = sorted(plugin_dir.glob(f"{manifest['id']}-*.zip"))
    if not zip_paths:
        return []
    zip_path = zip_paths[-1]
    zip_path_res = ROOT / "plugins" / zip_path.name
    if not zip_path_res.exists():
        return []
    return [{
        "id": manifest["id"],
        "name": manifest["name"],
        "version": manifest["version"],
        "description": manifest["description"],
        "minAppVersion": manifest.get("minAppVersion", 1),
        "entryClass": manifest.get("entryClass", ""),
        "capabilities": manifest.get("capabilities", []),
        "supportedFormats": manifest.get("supportedFormats", []),
        "runtime": "js",
        "abi": "",
        "abiVersion": manifest.get("abiVersion", 1),
        "sizeBytes": zip_path_res.stat().st_size,
        "installedSizeBytes": installed_size(zip_path_res),
        "checksum": sha256(zip_path_res),
        "downloadUrl": manifest.get("downloadUrl"),
        "jsEntry": manifest.get("jsEntry", "web/index.js"),
        "cssEntry": manifest.get("cssEntry", ""),
    }]


def build_entries(existing: list[dict]) -> list[dict]:
    kept = [e for e in existing if e.get("id") not in ("office-ooxml", "pptx-vanilla")]
    return kept + office_ooxml_entries() + pptx_vanilla_entries()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    existing = json.loads(CATALOG.read_text()).get("plugins", [])
    new_plugins = build_entries(existing)
    updated = {"plugins": new_plugins}

    if args.check:
        if json.dumps(updated, indent=2) == json.dumps(existing and {"plugins": existing}, indent=2):
            print("[catalog] up to date")
            return 0
        print("[catalog] OUT OF SYNC — run python3 scripts/update-catalog.py")
        return 1

    CATALOG.write_text(json.dumps(updated, indent=2) + "\n")
    for e in new_plugins:
        print(f"[catalog] {e['id']:<18} v{e['version']:<6} abi={e.get('abi') or '-':<10} "
              f"runtime={e.get('runtime') or '-':<22} {e['sizeBytes']}B sha={e['checksum'][:12]}…")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
