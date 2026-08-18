import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location(
    "android_maturation_conformance",
    ROOT / "scripts/android-maturation-conformance.py",
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class AndroidMaturationConformanceTests(unittest.TestCase):
    def test_night_mode_parser_preserves_supported_custom_modes(self):
        self.assertEqual(MODULE.night_mode_value("Night mode: auto"), "auto")
        self.assertEqual(MODULE.night_mode_value("Night mode: custom_schedule"), "custom_schedule")
        self.assertEqual(MODULE.night_mode_value("Night mode: custom_bedtime"), "custom_bedtime")
        with self.assertRaises(ValueError):
            MODULE.night_mode_value("Night mode: future-mode")

    def test_sha256_records_exact_artifact_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "artifact.apk"
            artifact.write_bytes(b"viewit")
            self.assertEqual(
                MODULE.sha256(artifact),
                "45ea79baeba50e2ea157e60b1dbc7e4c8af4462a9c85f98249677a3233bade1a",
            )


if __name__ == "__main__":
    unittest.main()
