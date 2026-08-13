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
import subprocess
import zipfile
from pathlib import Path

from release_io import atomic_write_text, persist_git_blob

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


UNIVERSAL_PLUGINS = ("office-universal", "compression-universal", "font-universal", "iwork-universal")


def universal_entries(plugin_ids=UNIVERSAL_PLUGINS) -> list[dict]:
    """Per-ABI catalog entries for the downloadable native universal plugins.

    These live under apps/mobile/plugins/<id> and are built by their build.sh
    into build/output/<id>-<version>-{arm64-v8a,x86_64}.zip. Since plugins are
    now fully downloadable (never baked into the APK), every ABI ships here.
    """
    out: list[dict] = []
    for pid in plugin_ids:
        mdir = ROOT / "apps" / "mobile" / "plugins" / pid
        manifest_file = mdir / "plugin.json"
        if not manifest_file.exists():
            continue
        manifest = json.loads(manifest_file.read_text())
        base = {
            "id": manifest["id"],
            "name": manifest["name"],
            "version": manifest["version"],
            "description": manifest["description"],
            "minAppVersion": manifest.get("minAppVersion", 1),
            "entryClass": manifest.get("entryClass", ""),
            "capabilities": manifest.get("capabilities", []),
            "base": manifest.get("base", "view"),
            "supportedFormats": manifest.get("supportedFormats", []),
            "runtime": manifest.get("runtime", "native"),
            "abiVersion": manifest.get("abiVersion", 1),
            "downloadUrl": manifest.get("downloadUrl") or "",
            "jsEntry": manifest.get("jsEntry", ""),
            "pptxVanilla": manifest.get("pptxVanilla", False),
        }
        for abi in ("arm64-v8a", "x86_64"):
            zip_path = mdir / "build" / "output" / f"{pid}-{manifest['version']}-{abi}.zip"
            if not zip_path.exists():
                raise SystemExit(f"[catalog] missing required artifact: {zip_path}")
            # The catalog host serves `<id>-<version>-<abi>.zip` (omnia proxy to
            # Pages); derive the URL from the packaged zip name, not the manifest
            # (which may only pin one ABI).
            abi_url = f"https://omnia.mihirpatil.co/plugins/{pid}-{manifest['version']}-{abi}.zip"
            out.append({
                **base,
                "abi": abi,
                "sizeBytes": zip_path.stat().st_size,
                "installedSizeBytes": installed_size(zip_path),
                "checksum": sha256(zip_path),
                "downloadUrl": abi_url,
            })
    return out


def player_base_entries() -> list[dict]:
    """Single js entry for the player-base plugin (base=play)."""
    manifest_path = ROOT / "plugins" / "player-base" / "plugin.json"
    zip_path = ROOT / "plugins" / f"player-base-{json.loads(manifest_path.read_text())['version']}.zip"
    if not manifest_path.exists() or not zip_path.exists():
        return []
    manifest = json.loads(manifest_path.read_text())
    return [{
        "id": manifest["id"],
        "name": manifest["name"],
        "version": manifest["version"],
        "description": manifest["description"],
        "minAppVersion": manifest.get("minAppVersion", 1),
        "entryClass": manifest.get("entryClass", ""),
        "capabilities": manifest.get("capabilities", []),
        "base": manifest.get("base", "view"),
        "supportedFormats": manifest.get("supportedFormats", []),
        "runtime": "js",
        "abi": "",
        "abiVersion": manifest.get("abiVersion", 1),
        "sizeBytes": zip_path.stat().st_size,
        "installedSizeBytes": installed_size(zip_path),
        "checksum": sha256(zip_path),
        "downloadUrl": f"https://omnia.mihirpatil.co/plugins/player-base-{manifest['version']}.zip",
        "jsEntry": manifest.get("jsEntry", "web/index.js"),
        "cssEntry": manifest.get("cssEntry", ""),
    }]


def editor_base_entries() -> list[dict]:
    """Single js entry for the editor-base plugin (base=edit)."""
    manifest_path = ROOT / "plugins" / "editor-base" / "plugin.json"
    zip_path = ROOT / "plugins" / f"editor-base-{json.loads(manifest_path.read_text())['version']}.zip"
    if not manifest_path.exists() or not zip_path.exists():
        return []
    manifest = json.loads(manifest_path.read_text())
    return [{
        "id": manifest["id"],
        "name": manifest["name"],
        "version": manifest["version"],
        "description": manifest["description"],
        "minAppVersion": manifest.get("minAppVersion", 1),
        "entryClass": manifest.get("entryClass", ""),
        "capabilities": manifest.get("capabilities", []),
        "base": manifest.get("base", "view"),
        "supportedFormats": manifest.get("supportedFormats", []),
        "runtime": "js",
        "abi": "",
        "abiVersion": manifest.get("abiVersion", 1),
        "sizeBytes": zip_path.stat().st_size,
        "installedSizeBytes": installed_size(zip_path),
        "checksum": sha256(zip_path),
        "downloadUrl": f"https://omnia.mihirpatil.co/plugins/editor-base-{manifest['version']}.zip",
        "jsEntry": manifest.get("jsEntry", "web/index.js"),
        "cssEntry": manifest.get("cssEntry", ""),
    }]


def build_entries(existing: list[dict], only: str | None = None) -> list[dict]:
    if only:
        rebuilt = {only}
        builders = {
            "office-universal": lambda: universal_entries(("office-universal",)),
            "compression-universal": lambda: universal_entries(("compression-universal",)),
            "font-universal": lambda: universal_entries(("font-universal",)),
            "iwork-universal": lambda: universal_entries(("iwork-universal",)),
            "office-ooxml": office_ooxml_entries,
            "pptx-vanilla": pptx_vanilla_entries,
            "player-base": player_base_entries,
            "editor-base": editor_base_entries,
        }
        if only not in builders:
            raise SystemExit(f"[catalog] unsupported --only plugin: {only}")
        kept = [entry for entry in existing if entry.get("id") not in rebuilt]
        return kept + builders[only]()
    rebuilt = {"office-ooxml", "pptx-vanilla", "player-base", "editor-base", *UNIVERSAL_PLUGINS}
    kept = [e for e in existing if e.get("id") not in rebuilt]
    return kept + office_ooxml_entries() + pptx_vanilla_entries() + universal_entries() + player_base_entries() + editor_base_entries()


def reject_replaced_versions(existing: list[dict], updated: list[dict]) -> None:
    old: dict[tuple, set[str]] = {}
    for entry in existing:
        key = (entry.get("id"), entry.get("version"), entry.get("abi") or "")
        old.setdefault(key, set()).add(entry.get("checksum"))
    history = subprocess.run(
        ["git", "log", "--format=%H", "--", str(CATALOG.relative_to(ROOT))],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    for commit in history.stdout.splitlines():
        snapshot = subprocess.run(
            ["git", "show", f"{commit}:{CATALOG.relative_to(ROOT)}"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        if snapshot.returncode != 0:
            continue
        for entry in json.loads(snapshot.stdout).get("plugins", []):
            key = (entry.get("id"), entry.get("version"), entry.get("abi") or "")
            old.setdefault(key, set()).add(entry.get("checksum"))
    for entry in updated:
        key = (entry.get("id"), entry.get("version"), entry.get("abi") or "")
        previous_checksums = old.get(key, set())
        if previous_checksums and entry.get("checksum") not in previous_checksums:
            plugin_id, version, abi = key
            raise SystemExit(
                f"[catalog] refusing to replace published bytes for {plugin_id} v{version} "
                f"abi={abi or '-'}; bump the plugin version"
            )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--only")
    ap.add_argument("--output", type=Path, default=CATALOG)
    args = ap.parse_args()

    existing = json.loads(CATALOG.read_text()).get("plugins", [])
    new_plugins = build_entries(existing, args.only)
    reject_replaced_versions(existing, new_plugins)
    updated = {"plugins": new_plugins}

    if args.check:
        if json.dumps(updated, indent=2) == json.dumps(existing and {"plugins": existing}, indent=2):
            print("[catalog] up to date")
            return 0
        print("[catalog] OUT OF SYNC — run python3 scripts/update-catalog.py")
        return 1

    atomic_write_text(args.output, json.dumps(updated, indent=2) + "\n")
    if args.output.resolve() == CATALOG.resolve():
        persist_git_blob(CATALOG)
    for e in new_plugins:
        print(f"[catalog] {e['id']:<18} v{e['version']:<6} abi={e.get('abi') or '-':<10} "
              f"runtime={e.get('runtime') or '-':<22} {e['sizeBytes']}B sha={e['checksum'][:12]}…")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
