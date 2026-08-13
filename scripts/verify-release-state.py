#!/usr/bin/env python3
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PLUGIN = ROOT / "apps/mobile/plugins/office-universal"


def run(*args: str) -> None:
    subprocess.run(args, cwd=ROOT, check=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify local office release invariants")
    parser.add_argument("--require-packages", action="store_true")
    args = parser.parse_args()

    manifest = json.loads((PLUGIN / "plugin.json").read_text())
    cargo = __import__("tomllib").load(open(PLUGIN / "Cargo.toml", "rb"))
    java = (PLUGIN / "src/main/java/ai/viewit/plugins/officeuniversal/OfficeUniversalPlugin.java").read_text()
    versions = {
        "plugin.json": manifest["version"],
        "Cargo.toml": cargo["package"]["version"],
    }
    if len(set(versions.values())) != 1:
        raise SystemExit("version drift: " + ", ".join(f"{key}={value}" for key, value in versions.items()))
    if not re.search(r'native\s+String\s+getVersion\(\)', java):
        raise SystemExit("Java wrapper must use native getVersion()")

    version = manifest["version"]
    if args.require_packages:
        for abi in ("arm64-v8a", "x86_64"):
            package = PLUGIN / "build/output" / f"office-universal-{version}-{abi}.zip"
            run(
                sys.executable,
                "scripts/validate-plugin-package.py",
                str(package),
                "--id",
                "office-universal",
                "--version",
                version,
                "--abi",
                abi,
            )

    for path in (ROOT / "plugins/catalog.json", ROOT / "plugins/catalog.signed.json"):
        object_id = subprocess.run(
            ["git", "hash-object", str(path)],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        run("git", "cat-file", "-e", f"{object_id}^{{blob}}")

    run(sys.executable, "scripts/sign-catalog.py", "verify")
    subprocess.run(["git", "write-tree"], cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    print(f"[release-state] OK office-universal v{version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
