#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import urllib.request
from pathlib import Path


def artifact_name(entry: dict) -> str:
    return entry["downloadUrl"].rsplit("/", 1)[-1]


def checksum(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def find_artifact(name: str, expected_checksum: str, roots: list[Path]) -> Path | None:
    for root in roots:
        direct = root / name
        candidates = [direct] if direct.is_file() else []
        if root.is_dir():
            candidates.extend(root.glob(f"**/{name}"))
        for candidate in candidates:
            if candidate.is_file() and checksum(candidate) == expected_checksum:
                return candidate
    return None


def stage_artifacts(
    catalog_path: Path,
    signed_catalog_path: Path,
    destination: Path,
    search_roots: list[Path],
    allow_downloads: bool = True,
) -> int:
    catalog = json.loads(catalog_path.read_text())
    signed = json.loads(signed_catalog_path.read_text())
    if signed.get("catalog") != catalog:
        raise ValueError("staged signed/unsigned catalogs differ")

    destination.mkdir(parents=True, exist_ok=True)
    for entry in catalog["plugins"]:
        name = artifact_name(entry)
        output = destination / name
        source = find_artifact(name, entry["checksum"], search_roots)
        if source:
            shutil.copy2(source, output)
        elif allow_downloads:
            request = urllib.request.Request(
                entry["downloadUrl"], headers={"User-Agent": "viewit-release-stager"}
            )
            with urllib.request.urlopen(request, timeout=60) as response:
                output.write_bytes(response.read())
        else:
            raise FileNotFoundError(f"no matching local artifact: {name}")

        data = output.read_bytes()
        if len(data) != entry["sizeBytes"] or hashlib.sha256(data).hexdigest() != entry["checksum"]:
            raise ValueError(f"staged artifact mismatch: {output}")
    return len(catalog["plugins"])


def main() -> int:
    parser = argparse.ArgumentParser(description="Stage and verify catalog plugin artifacts")
    parser.add_argument("--catalog", type=Path, required=True)
    parser.add_argument("--signed-catalog", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--search-root", type=Path, action="append", default=[])
    parser.add_argument("--no-downloads", action="store_true")
    args = parser.parse_args()
    count = stage_artifacts(
        args.catalog,
        args.signed_catalog,
        args.destination,
        args.search_root,
        not args.no_downloads,
    )
    print(f"[publish] staged and verified {count} catalog artifacts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
