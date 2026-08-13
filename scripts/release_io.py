from __future__ import annotations

import os
import subprocess
import tempfile
from pathlib import Path


def atomic_write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def persist_git_blob(path: Path) -> None:
    """Write generated content to Git's object store before external tools stage it."""
    root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        cwd=path.parent,
        check=False,
        capture_output=True,
        text=True,
    )
    if root.returncode != 0:
        return
    object_id = subprocess.run(
        ["git", "hash-object", "-w", str(path)],
        cwd=root.stdout.strip(),
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    subprocess.run(
        ["git", "cat-file", "-e", f"{object_id}^{{blob}}"],
        cwd=root.stdout.strip(),
        check=True,
    )
