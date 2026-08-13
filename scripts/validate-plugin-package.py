#!/usr/bin/env python3
import argparse
from collections import Counter
import json
import zipfile
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate a packaged ViewIt plugin ZIP")
    parser.add_argument("zip", type=Path)
    parser.add_argument("--id", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--abi", required=True)
    args = parser.parse_args()

    with zipfile.ZipFile(args.zip) as archive:
        name_list = archive.namelist()
        names = set(name_list)
        duplicates = sorted(name for name, count in Counter(name_list).items() if count > 1)
        if duplicates:
            raise SystemExit(f"{args.zip}: duplicate entries: {', '.join(duplicates)}")
        unsafe = sorted(
            name
            for name in names
            if name.startswith(("/", "\\")) or ".." in Path(name).parts or "\\" in name
        )
        if unsafe:
            raise SystemExit(f"{args.zip}: unsafe entries: {', '.join(unsafe)}")
        manifest = json.loads(archive.read("plugin.json"))
        required = {
            "plugin.json",
            "dex/classes.dex",
            f"lib/{args.abi}/libviewit_plugin_{args.id.replace('-', '_')}.so",
        }
        if manifest.get("jsEntry"):
            required.add(manifest["jsEntry"])
        missing = sorted(required - names)
        if missing:
            raise SystemExit(f"{args.zip}: missing required entries: {', '.join(missing)}")
        if manifest.get("id") != args.id or manifest.get("version") != args.version:
            raise SystemExit(f"{args.zip}: manifest id/version does not match package name")
        native_libs = [name for name in names if name.endswith(".so")]
        if native_libs != [f"lib/{args.abi}/libviewit_plugin_{args.id.replace('-', '_')}.so"]:
            raise SystemExit(f"{args.zip}: unexpected native libraries: {native_libs}")
        for required_entry in required - {"plugin.json"}:
            if archive.getinfo(required_entry).file_size == 0:
                raise SystemExit(f"{args.zip}: empty required entry: {required_entry}")

    print(f"[validate] OK {args.id} v{args.version} abi={args.abi}: {args.zip}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
