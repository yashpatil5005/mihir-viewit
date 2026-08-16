import importlib.util
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "patch-android-open-with.py"
spec = importlib.util.spec_from_file_location("patch_android_open_with", SCRIPT)
module = importlib.util.module_from_spec(spec)
assert spec.loader
spec.loader.exec_module(module)


class AndroidOpenWithPatchTests(unittest.TestCase):
    def test_adds_generic_view_filter_once(self):
        with tempfile.TemporaryDirectory() as temporary:
            manifest = Path(temporary) / "AndroidManifest.xml"
            manifest.write_text("<manifest><application><activity>\n        </activity></application></manifest>")
            self.assertTrue(module.patch_manifest(manifest))
            self.assertFalse(module.patch_manifest(manifest))
            body = manifest.read_text()
            self.assertEqual(body.count(module.MARKER), 1)
            self.assertEqual(body.count(module.WORKER_MARKER), 1)
            self.assertIn('android:process=":media_worker"', body)
            self.assertEqual(body.count('android.intent.action.VIEW'), 2)
            self.assertIn('android:mimeType="*/*"', body)
            self.assertIn('android:scheme="content"', body)
            self.assertIn('android:scheme="file"', body)

    def test_rejects_manifest_without_activity(self):
        with tempfile.TemporaryDirectory() as temporary:
            manifest = Path(temporary) / "AndroidManifest.xml"
            manifest.write_text("<manifest />")
            with self.assertRaisesRegex(RuntimeError, "MainActivity"):
                module.patch_manifest(manifest)

    def test_upgrades_an_existing_typed_only_filter(self):
        with tempfile.TemporaryDirectory() as temporary:
            manifest = Path(temporary) / "AndroidManifest.xml"
            typed_only = module.FILTER.rsplit("            <intent-filter>", 1)[0]
            manifest.write_text(
                "<manifest><application><activity>\n"
                + typed_only
                + "        </activity></application></manifest>"
            )
            self.assertTrue(module.patch_manifest(manifest))
            body = manifest.read_text()
            self.assertEqual(body.count(module.MARKER), 1)
            self.assertEqual(body.count('android.intent.action.VIEW'), 2)

    def test_standard_android_commands_apply_overlay(self):
        root_package = (ROOT / "package.json").read_text()
        mobile_package = (ROOT / "apps/mobile/package.json").read_text()
        prepare = (ROOT / "scripts/prepare-android-project.sh").read_text()
        self.assertIn("prepare-android-project.sh", root_package)
        self.assertIn("prepare-android-project.sh", mobile_package)
        self.assertIn("android init", prepare)
        self.assertIn("patch-android-mainactivity.sh", prepare)


if __name__ == "__main__":
    unittest.main()
