#!/usr/bin/env python3
"""Ensure Android admits every file to ViewIt's runtime capability resolver."""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "apps/mobile/src-tauri/gen/android/app/src/main/AndroidManifest.xml"
MARKER = "<!-- viewit-runtime-format-admission -->"
WORKER_MARKER = "<!-- viewit-media-worker -->"
FILTER = f"""            {MARKER}
            <intent-filter>
                <action android:name="android.intent.action.VIEW" />
                <category android:name="android.intent.category.DEFAULT" />
                <data android:scheme="content" />
                <data android:scheme="file" />
                <data android:mimeType="*/*" />
            </intent-filter>
            <intent-filter>
                <action android:name="android.intent.action.VIEW" />
                <category android:name="android.intent.category.DEFAULT" />
                <data android:scheme="content" />
                <data android:scheme="file" />
            </intent-filter>
"""
WORKER = f"""        {WORKER_MARKER}
        <service
            android:name=".MediaWorkerService"
            android:exported="false"
            android:process=":media_worker"
            android:stopWithTask="true" />
"""


def patch_manifest(path: Path) -> bool:
    body = path.read_text()
    changed = False
    if FILTER not in body:
        activity_end = body.find("        </activity>")
        if activity_end < 0:
            raise RuntimeError("MainActivity closing tag not found")
        marker_start = body.find(f"            {MARKER}")
        insert_at = marker_start if marker_start >= 0 else activity_end
        body = body[:insert_at] + FILTER + body[activity_end:]
        changed = True
    if WORKER not in body:
        application_end = body.find("    </application>")
        if application_end < 0:
            application_end = body.find("</application>")
        if application_end < 0:
            raise RuntimeError("Application closing tag not found")
        body = body[:application_end] + WORKER + body[application_end:]
        changed = True
    if changed:
        path.write_text(body)
    return changed


def main() -> int:
    if not MANIFEST.is_file():
        print(f"[patch-android] missing {MANIFEST}", file=sys.stderr)
        return 1
    changed = patch_manifest(MANIFEST)
    print(f"[patch-android] Open with runtime admission {'installed' if changed else 'already present'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
