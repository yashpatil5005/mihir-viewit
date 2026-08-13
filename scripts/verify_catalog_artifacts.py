#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def find_named_artifact(name: str, roots: list[Path]) -> Path | None:
    for root in roots:
        direct = root / name
        if direct.is_file():
            return direct
        if root.is_dir():
            match = next((path for path in root.glob(f"**/{name}") if path.is_file()), None)
            if match:
                return match
    return None


def verify_local_artifacts(
    catalog_path: Path,
    roots: list[Path],
    required_ids: set[str],
    require_all: bool = False,
) -> int:
    entries = json.loads(catalog_path.read_text())["plugins"]
    failures = 0
    present_ids = {entry["id"] for entry in entries}
    for missing_id in sorted(required_ids - present_ids):
        print(f"  FAIL  required plugin absent from catalog: {missing_id}")
        failures += 1
    for entry in entries:
        name = entry["downloadUrl"].rsplit("/", 1)[-1]
        artifact = find_named_artifact(name, roots)
        tag = f"{entry['id']:<22} {entry.get('abi') or '-':<10}"
        if artifact is None:
            if require_all or entry["id"] in required_ids:
                print(f"  FAIL  {tag} missing {name}")
                failures += 1
            else:
                print(f"  SKIP  {tag} artifact not local")
            continue
        data = artifact.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        if len(data) != entry["sizeBytes"] or digest != entry["checksum"]:
            print(f"  FAIL  {tag} size/checksum mismatch: {artifact}")
            failures += 1
        else:
            print(f"  OK    {tag} {artifact}")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify exact local artifacts against catalog")
    parser.add_argument("--catalog", type=Path, required=True)
    parser.add_argument("--search-root", type=Path, action="append", default=[])
    parser.add_argument("--require-id", action="append", default=[])
    parser.add_argument("--require-all", action="store_true")
    args = parser.parse_args()
    failures = verify_local_artifacts(
        args.catalog, args.search_root, set(args.require_id), args.require_all
    )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
