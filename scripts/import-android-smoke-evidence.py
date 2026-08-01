#!/usr/bin/env python3
"""Import ViewIt Android smoke screenshots and build queryable 3x3 grids.

The smoke runner writes evidence to the connected device. This script replaces
the local evidence directory, pulls that device directory, creates 3x3 image
grids, and writes one combined JSON file that maps each grid cell to its file,
metadata, and screenshot path.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any

from PIL import Image, ImageDraw, ImageFont


DEFAULT_DEVICE_OUTPUT_DIR = "/sdcard/Download/viewit_smoke_evidence"
DEFAULT_LOCAL_OUTPUT_DIR = ".agent/testing/smokeEvidence"
GRID_COLS = 3
GRID_ROWS = 3
CELL_W = 720
CELL_H = 1280
LABEL_H = 80


def adb_cmd(serial: str | None) -> list[str]:
    cmd = ["adb"]
    if serial:
        cmd += ["-s", serial]
    return cmd


def sh(args: list[str], *, check: bool = True) -> str:
    res = subprocess.run(args, check=check, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return res.stdout


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--serial", default=os.environ.get("ADB_SERIAL") or os.environ.get("ANDROID_SERIAL", ""))
    p.add_argument("--device-output-dir", default=os.environ.get("SMOKE_DEVICE_OUTPUT_DIR", DEFAULT_DEVICE_OUTPUT_DIR))
    p.add_argument("--local-output-dir", default=os.environ.get("SMOKE_LOCAL_OUTPUT_DIR", DEFAULT_LOCAL_OUTPUT_DIR))
    return p.parse_args()


def load_json(path: Path) -> Any:
    with path.open() as f:
        return json.load(f)


def fit_image(path: Path, size: tuple[int, int]) -> Image.Image:
    image = Image.open(path).convert("RGB")
    image.thumbnail(size, Image.Resampling.LANCZOS)
    canvas = Image.new("RGB", size, "#111111")
    x = (size[0] - image.width) // 2
    y = (size[1] - image.height) // 2
    canvas.paste(image, (x, y))
    return canvas


def draw_label(draw: ImageDraw.ImageDraw, xy: tuple[int, int], text: str) -> None:
    font = ImageFont.load_default()
    x, y = xy
    lines = []
    current = ""
    for part in text.split(" "):
        candidate = f"{current} {part}".strip()
        if len(candidate) > 42:
            if current:
                lines.append(current)
            current = part
        else:
            current = candidate
    if current:
        lines.append(current)
    for idx, line in enumerate(lines[:4]):
        draw.text((x, y + idx * 13), line, fill="#ffffff", font=font)


def build_grids(local_dir: Path) -> dict[str, Any]:
    manifest_path = local_dir / "manifest.json"
    manifest = load_json(manifest_path)
    screenshots_dir = local_dir / "screenshots"
    grids_dir = local_dir / "grids"
    grids_dir.mkdir(parents=True, exist_ok=True)

    results = sorted(manifest.get("results", []), key=lambda r: r.get("index", 0))
    combined: dict[str, Any] = {
        "source_manifest": str(manifest_path),
        "device_output_dir": manifest.get("device_output_dir"),
        "fixture_dir": manifest.get("fixture_dir"),
        "settle_seconds": manifest.get("settle_seconds"),
        "total": len(results),
        "grid": {"cols": GRID_COLS, "rows": GRID_ROWS, "cell_width": CELL_W, "cell_height": CELL_H, "label_height": LABEL_H},
        "grids": [],
        "items": [],
        "review_instructions": manifest.get("review_instructions", []),
    }

    per_grid = GRID_COLS * GRID_ROWS
    for grid_index, start in enumerate(range(0, len(results), per_grid), start=1):
        chunk = results[start:start + per_grid]
        grid_image = Image.new("RGB", (GRID_COLS * CELL_W, GRID_ROWS * CELL_H), "#222222")
        draw = ImageDraw.Draw(grid_image)
        grid_name = f"grid-{grid_index:03d}.png"
        grid_path = grids_dir / grid_name

        cells = []
        for offset, result in enumerate(chunk):
            row = offset // GRID_COLS
            col = offset % GRID_COLS
            x = col * CELL_W
            y = row * CELL_H
            screenshot_info = result.get("screenshot") or {}
            remote_path = screenshot_info.get("path", "")
            screenshot_name = Path(remote_path).name if remote_path else f"{result.get('index', 0):03d}_{result.get('file', 'missing')}.png"
            screenshot_path = screenshots_dir / screenshot_name

            draw.rectangle((x, y, x + CELL_W - 1, y + CELL_H - 1), outline="#555555")
            draw.rectangle((x, y, x + CELL_W, y + LABEL_H), fill="#000000")
            status = "PASS" if result.get("ok") else "FAIL"
            label = f"{result.get('index')}: {status} r{row+1}c{col+1} {result.get('file')}"
            draw_label(draw, (x + 6, y + 6), label)

            if screenshot_path.exists():
                fitted = fit_image(screenshot_path, (CELL_W, CELL_H - LABEL_H))
                grid_image.paste(fitted, (x, y + LABEL_H))
            else:
                draw_label(draw, (x + 6, y + LABEL_H + 8), f"missing screenshot: {screenshot_name}")

            item = {
                "grid": grid_name,
                "grid_index": grid_index,
                "cell_index": offset + 1,
                "row": row + 1,
                "col": col + 1,
                "file": result.get("file"),
                "ok": result.get("ok"),
                "failures": result.get("failures", []),
                "metrics": result.get("metrics"),
                "screenshot": str(screenshot_path),
                "metadata": result,
            }
            cells.append(item)
            combined["items"].append(item)

        grid_image.save(grid_path)
        combined["grids"].append({"grid": grid_name, "path": str(grid_path), "cells": cells})

    combined_path = local_dir / "grid-metadata.json"
    with combined_path.open("w") as f:
        json.dump(combined, f, indent=2, sort_keys=True)
    return combined


def main() -> int:
    args = parse_args()
    adb = adb_cmd(args.serial)
    local_dir = Path(args.local_output_dir)
    if local_dir.exists():
        shutil.rmtree(local_dir)
    local_dir.parent.mkdir(parents=True, exist_ok=True)
    sh(adb + ["pull", args.device_output_dir, str(local_dir)])
    combined = build_grids(local_dir)
    print(f"[import-smoke] imported to {local_dir}")
    for grid in combined["grids"]:
        print(f"[import-smoke] grid {grid['grid']}: {grid['path']}")
    print(f"[import-smoke] metadata: {local_dir / 'grid-metadata.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
