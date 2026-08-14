import unittest
from unittest.mock import MagicMock, patch
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))

import importlib.util

spec = importlib.util.spec_from_file_location("android_intent_tests", str(SCRIPTS / "android-intent-tests.py"))
intent_module = importlib.util.module_from_spec(spec)
sys.modules["android_intent_tests"] = intent_module
spec.loader.exec_module(intent_module)


class TestAndroidIntentTests(unittest.TestCase):
    def test_query_activities_parses_packages(self):
        mock_session = MagicMock()
        mock_session.shell.return_value = """
        2 activities found:
          Activity #0:
            ActivityInfo:
              name=ai.viewit.app.MainActivity
              packageName=ai.viewit.app
          Activity #1:
            ActivityInfo:
              name=com.other.app.ViewerActivity
              packageName=com.other.app
        """
        pkgs = intent_module.query_activities(mock_session, "content://test/file.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        self.assertEqual(pkgs, ["ai.viewit.app", "com.other.app"])

    def test_query_activities_empty(self):
        mock_session = MagicMock()
        mock_session.shell.return_value = "0 activities found:"
        pkgs = intent_module.query_activities(mock_session, "content://test/unknown", None)
        self.assertEqual(pkgs, [])


if __name__ == "__main__":
    unittest.main()
