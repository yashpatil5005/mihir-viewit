#!/usr/bin/env python3
"""Verify that R8 preserves the host interfaces implemented by plugin DEX files."""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path


EXPECTED = {
    "ai.viewit.app.ViewItDocumentPlugin": {
        "getId": "()Ljava/lang/String;", "getVersion": "()Ljava/lang/String;",
        "getSupportedFormats": "()Ljava/util/List;", "initialize": "(Landroid/content/Context;)V",
        "canHandle": "(Ljava/lang/String;)Z", "canHandleExt": "(Ljava/lang/String;)Z",
        "render": "(Landroid/net/Uri;Ljava/lang/String;)Ljava/lang/String;", "cleanup": "()V",
    },
    "ai.viewit.app.ViewItPlugin": {
        "getId": "()Ljava/lang/String;", "getVersion": "()Ljava/lang/String;",
        "getSupportedFormats": "()Ljava/util/List;", "initialize": "(Landroid/content/Context;)V",
        "canHandle": "(Ljava/lang/String;)Z", "canHandleExt": "(Ljava/lang/String;)Z",
        "transcode": "(Landroid/net/Uri;Ljava/io/File;Lai/viewit/app/PluginProgress;)Z", "cleanup": "()V",
    },
    "ai.viewit.app.ArchivePlugin": {
        "listArchive": "(Landroid/net/Uri;Ljava/lang/String;)Ljava/lang/String;",
        "extractEntry": "(Landroid/net/Uri;Ljava/lang/String;Ljava/lang/String;)[B",
        "extractEntryToFile": "(Landroid/net/Uri;Ljava/lang/String;Ljava/lang/String;Ljava/io/File;)Ljava/lang/String;",
        "extractAll": "(Landroid/net/Uri;Ljava/lang/String;Ljava/io/File;)Ljava/lang/String;",
        "detectFormat": "(Landroid/net/Uri;Ljava/lang/String;)Ljava/lang/String;",
    },
    "ai.viewit.app.PluginProgress": {"update": "(F)V"},
}


def mapped_interfaces(mapping: str) -> dict[str, dict[str, str]]:
    result: dict[str, dict[str, str]] = {}
    current: str | None = None
    for line in mapping.splitlines():
        if line and not line.startswith((" ", "#")) and line.endswith(":") and " -> " in line:
            source, target = line[:-1].split(" -> ", 1)
            current = source if source in EXPECTED else None
            if current is not None:
                result[current] = {"__class__": target}
            continue
        if current is None or not line.startswith("    ") or " -> " not in line or line.lstrip().startswith("#"):
            continue
        signature, target = line.strip().rsplit(" -> ", 1)
        match = re.search(r"(?:^|:)\S+\s+([A-Za-z_$][\w$]*)\(", signature)
        if match:
            result[current][match.group(1)] = target
    return result


def validate_mapping(mapping: str) -> list[str]:
    parsed = mapped_interfaces(mapping)
    failures: list[str] = []
    for interface, methods in EXPECTED.items():
        members = parsed.get(interface)
        if members is None:
            failures.append(f"missing mapping block for {interface}")
            continue
        if members.get("__class__") != interface:
            failures.append(f"class renamed: {interface} -> {members.get('__class__')}")
        for method in sorted(methods):
            target = members.get(method)
            # R8 omits unchanged members from mapping.txt.
            if target is not None and target != method:
                failures.append(f"method renamed: {interface}.{method} -> {target}")
    return failures


def parse_javap(output: str) -> dict[str, dict[str, str]]:
    result: dict[str, dict[str, str]] = {}
    current: str | None = None
    pending_method: str | None = None
    for line in output.splitlines():
        class_match = re.match(r"public interface (ai\.viewit\.app\.\w+) \{", line)
        if class_match:
            current = class_match.group(1)
            result[current] = {}
            continue
        method_match = re.match(r"\s+public abstract .+ ([A-Za-z_$][\w$]*)\(.*\);", line)
        if current and method_match:
            pending_method = method_match.group(1)
            continue
        descriptor_match = re.match(r"\s+descriptor: (\S+)", line)
        if current and pending_method and descriptor_match:
            result[current][pending_method] = descriptor_match.group(1)
            pending_method = None
    return result


def validate_descriptors(output: str) -> list[str]:
    parsed = parse_javap(output)
    failures: list[str] = []
    for interface, methods in EXPECTED.items():
        actual = parsed.get(interface)
        if actual is None:
            failures.append(f"missing compiled interface: {interface}")
            continue
        for method, descriptor in methods.items():
            if method not in actual:
                failures.append(f"missing ABI method: {interface}.{method}{descriptor}")
            elif actual[method] != descriptor:
                failures.append(f"descriptor changed: {interface}.{method} {descriptor} -> {actual[method]}")
        extras = sorted(set(actual) - set(methods))
        failures.extend(f"unexpected ABI method: {interface}.{method}{actual[method]}" for method in extras)
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mapping", type=Path)
    parser.add_argument("classes", type=Path)
    args = parser.parse_args()
    if not args.mapping.is_file():
        raise SystemExit(f"plugin ABI mapping not found: {args.mapping}")
    if not args.classes.is_dir():
        raise SystemExit(f"plugin ABI classes not found: {args.classes}")
    javap = subprocess.run(
        ["javap", "-classpath", str(args.classes), "-s", "-p", *EXPECTED],
        check=False, capture_output=True, text=True,
    )
    if javap.returncode != 0:
        raise SystemExit(f"plugin ABI javap failed: {javap.stderr.strip()}")
    failures = validate_descriptors(javap.stdout) + validate_mapping(args.mapping.read_text())
    if failures:
        raise SystemExit("plugin ABI verification failed:\n- " + "\n- ".join(failures))
    print(f"[android] plugin ABI preserved: {args.mapping}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
