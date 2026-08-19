import importlib.util
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location(
    "android_media_worker_conformance",
    ROOT / "scripts/android-media-worker-conformance.py",
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(MODULE)

FORMAT_SPEC = importlib.util.spec_from_file_location(
    "android_format_smoke",
    ROOT / "scripts/android-format-smoke.py",
)
FORMAT_MODULE = importlib.util.module_from_spec(FORMAT_SPEC)
assert FORMAT_SPEC.loader
sys.modules[FORMAT_SPEC.name] = FORMAT_MODULE
FORMAT_SPEC.loader.exec_module(FORMAT_MODULE)


class MediaWorkerConformanceTests(unittest.TestCase):
    def test_progress_must_be_monotonic_and_complete(self):
        MODULE.validate_progress([0.01, 0.4, 1.0])
        with self.assertRaises(ValueError):
            MODULE.validate_progress([0.1, 0.05, 1.0])
        with self.assertRaises(ValueError):
            MODULE.validate_progress([0.1, 0.8])

    def test_video_and_audio_output_contracts(self):
        MODULE.validate_output({
            "sizeBytes": 10,
            "streams": [{"mime": "video/avc"}, {"mime": "audio/mp4a-latm"}],
        })
        MODULE.validate_output({
            "sizeBytes": 10,
            "streams": [{"mime": "audio/mp4a-latm"}],
        }, audio_only=True)
        with self.assertRaises(ValueError):
            MODULE.validate_output({"sizeBytes": 10, "streams": [{"mime": "video/mp4v-es"}]})

    def test_worker_memory_parser_requires_total_pss(self):
        self.assertEqual(MODULE.parse_total_pss("TOTAL PSS:    40497"), 40497)
        with self.assertRaises(ValueError):
            MODULE.parse_total_pss("No process found")


class AndroidFormatSmokeScriptTests(unittest.TestCase):
    def test_internal_contract_diagnostics_fail_visual_smoke(self):
        self.assertEqual(
            FORMAT_MODULE._no_error({"hasInternalDiagnostic": True}),
            "internal diagnostic text visible in DOM",
        )

    def test_archive_preview_reaches_digest(self):
        self.assertNotIn(
            "      }\n      return true;\n      try {",
            FORMAT_MODULE.PREVIEW_SHA_JS,
        )
        self.assertIn("__TAURI__.core.invoke('archive_extract'", FORMAT_MODULE.PREVIEW_SHA_JS)

    def test_native_player_click_uses_json_quoted_text(self):
        expressions = []

        class Client:
            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return None

            def evaluate(self, expression):
                expressions.append(expression)
                return True

        original = FORMAT_MODULE.CdpClient
        FORMAT_MODULE.CdpClient = lambda _url, _timeout: Client()
        try:
            self.assertTrue(FORMAT_MODULE.ws_click_text("ws://example", "Native Android player"))
        finally:
            FORMAT_MODULE.CdpClient = original

        self.assertIn('includes("Native Android player")', expressions[0])


if __name__ == "__main__":
    unittest.main()
