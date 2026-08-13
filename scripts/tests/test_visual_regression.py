import importlib.util
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

try:
    from PIL import Image
except ImportError as error:
    raise unittest.SkipTest("Pillow is optional; visual device tests are not part of host CI") from error


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
TEST_TMP = ROOT / "build/visual-test-tmp"
TEST_TMP.mkdir(parents=True, exist_ok=True)
os.environ["TMPDIR"] = str(TEST_TMP)
tempfile.tempdir = str(TEST_TMP)
sys.path.insert(0, str(SCRIPTS))

from visual_diff import compare_images


def load_visual_runner():
    path = SCRIPTS / "android-visual-regression.py"
    spec = importlib.util.spec_from_file_location("android_visual_regression", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


visual_runner = load_visual_runner()


class ImageDiffTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def image(self, name: str, color: tuple[int, int, int, int]) -> Path:
        path = self.root / name
        Image.new("RGBA", (10, 10), color).save(path)
        return path

    def test_identical_images_pass(self):
        baseline = self.image("baseline.png", (10, 20, 30, 255))
        candidate = self.image("candidate.png", (10, 20, 30, 255))
        result = compare_images(baseline, candidate, self.root / "diff.png")
        self.assertTrue(result.passed)
        self.assertEqual(result.changed_pixels, 0)

    def test_changed_ratio_respects_pixel_threshold(self):
        baseline = self.image("baseline.png", (0, 0, 0, 255))
        candidate = self.image("candidate.png", (0, 0, 0, 255))
        image = Image.open(candidate).convert("RGBA")
        image.putpixel((0, 0), (255, 255, 255, 255))
        image.save(candidate)

        result = compare_images(
            baseline,
            candidate,
            self.root / "diff.png",
            max_changed_ratio=0.005,
        )

        self.assertFalse(result.passed)
        self.assertEqual(result.changed_pixels, 1)
        self.assertAlmostEqual(result.changed_ratio, 0.01)

    def test_dimension_mismatch_is_rejected(self):
        baseline = self.image("baseline.png", (0, 0, 0, 255))
        candidate = self.root / "candidate.png"
        Image.new("RGBA", (11, 10), (0, 0, 0, 255)).save(candidate)
        with self.assertRaisesRegex(ValueError, "image dimensions differ"):
            compare_images(baseline, candidate, self.root / "diff.png")


class VisualRunnerTests(unittest.TestCase):
    def test_profile_name_includes_render_critical_device_values(self):
        profile = {
            "model": "Galaxy Test",
            "android": "16",
            "size": "Physical size: 1080x2340",
            "density": "Override density: 420",
            "font_scale": "1.0",
            "webview_version": "140.0.1",
            "app_version": "0.2.0",
            "locale": "en-IN",
            "build_fingerprint": "vendor/device/build",
        }
        value = visual_runner.profile_name(profile, (1082, 2342), True)
        self.assertIn("galaxy-test", value)
        self.assertIn("1080x2340", value)
        self.assertIn("d420", value)
        self.assertIn("wv140.0.1", value)
        self.assertIn("build", value)
        self.assertIn("dark", value)
        self.assertIn("capture1082x2342", value)

    def test_default_cases_have_unique_names_and_ready_conditions(self):
        names = [case.name for case in visual_runner.DEFAULT_CASES]
        self.assertEqual(len(names), len(set(names)))
        self.assertTrue(all(case.ready and case.path.startswith("/sdcard/") for case in visual_runner.DEFAULT_CASES))


if __name__ == "__main__":
    unittest.main()
