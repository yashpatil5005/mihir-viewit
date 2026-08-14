import importlib.util
import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))


def load_runner():
    path = SCRIPTS / "android-plugin-lifecycle.py"
    spec = importlib.util.spec_from_file_location("android_plugin_lifecycle", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


runner = load_runner()


class AndroidPluginLifecycleTests(unittest.TestCase):
    def test_adversarial_plugins_cover_id_and_zip_traversal(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixtures = runner.write_adversarial_plugins(Path(temporary))
            with zipfile.ZipFile(fixtures["unsafe-id"]) as archive:
                manifest = json.loads(archive.read("plugin.json"))
                self.assertEqual(manifest["id"], "../unsafe-plugin")
            with zipfile.ZipFile(fixtures["zip-traversal"]) as archive:
                self.assertIn("../escaped.txt", archive.namelist())
            with zipfile.ZipFile(fixtures["checksum-mismatch"]) as archive:
                manifest = json.loads(archive.read("plugin.json"))
                self.assertEqual(manifest["checksum"], "0" * 64)

    def test_install_expression_has_terminal_timeout_and_callback_filter(self):
        expression = runner.install_expression("/sdcard/plugin.zip", "callback-id", 1234)
        self.assertIn("event: 'timeout'", expression)
        self.assertIn("event.id !== id", expression)
        self.assertIn("installPluginLocal", expression)
        self.assertIn("1234", expression)
        self.assertIn("callbackRestored", expression)
        self.assertIn("catch (error)", expression)

    def test_render_expression_restores_document_callback(self):
        expression = runner.render_expression("file:///sample.docx")
        self.assertIn("window._documentPluginCallback = previous", expression)
        self.assertIn("renderDocumentWithPlugin", expression)
        self.assertIn("callbacksRestored", expression)

    def test_output_must_remain_under_build_directory(self):
        with self.assertRaisesRegex(runner.LifecycleFailure, "must be a child"):
            runner.validate_output_path(ROOT)

    def test_existing_pinned_fixture_must_match_hash(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "fixture.zip"
            path.write_bytes(b"unexpected")
            previous = runner.PINNED_ARTIFACTS
            try:
                runner.PINNED_ARTIFACTS = {path: ("https://example.invalid/fixture.zip", "0" * 64)}
                with self.assertRaisesRegex(runner.LifecycleFailure, "checksum mismatch"):
                    runner.ensure_pinned_artifact(path)
            finally:
                runner.PINNED_ARTIFACTS = previous


if __name__ == "__main__":
    unittest.main()
