import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
import warnings
import zipfile
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
TEST_TMP = ROOT / "build/release-test-tmp"
TEST_TMP.mkdir(parents=True, exist_ok=True)
os.environ["TMPDIR"] = str(TEST_TMP)
tempfile.tempdir = str(TEST_TMP)
sys.path.insert(0, str(SCRIPTS))

from release_io import atomic_write_text, persist_git_blob


def load_script(name: str):
    path = SCRIPTS / name
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader
    spec.loader.exec_module(module)
    return module


update_catalog = load_script("update-catalog.py")
stage_plugin_catalog = load_script("stage_plugin_catalog.py")
verify_catalog_artifacts = load_script("verify_catalog_artifacts.py")


class AtomicWriteTests(unittest.TestCase):
    def test_atomic_write_replaces_content_without_temp_files(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "catalog.json"
            path.write_text("old")

            atomic_write_text(path, "new\n")

            self.assertEqual(path.read_text(), "new\n")
            self.assertEqual(list(path.parent.glob(f".{path.name}.*")), [])

    def test_persist_git_blob_writes_reachable_object(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            path = root / "generated.json"
            path.write_text('{"ok":true}\n')

            persist_git_blob(path)

            object_id = subprocess.run(
                ["git", "hash-object", str(path)],
                cwd=root,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            subprocess.run(["git", "cat-file", "-e", f"{object_id}^{{blob}}"], cwd=root, check=True)


class CatalogTests(unittest.TestCase):
    def test_scoped_build_preserves_unrelated_entries(self):
        existing = [
            {"id": "office-universal", "version": "1", "abi": "arm64-v8a"},
            {"id": "font-universal", "version": "2", "abi": "arm64-v8a"},
        ]
        replacement = [{"id": "office-universal", "version": "3", "abi": "arm64-v8a"}]

        with patch.object(update_catalog, "universal_entries", return_value=replacement):
            result = update_catalog.build_entries(existing, "office-universal")

        self.assertEqual(result, [existing[1], replacement[0]])

    def test_rejects_changed_checksum_for_existing_version(self):
        existing = [
            {
                "id": "office-universal",
                "version": "1.0.0",
                "abi": "arm64-v8a",
                "checksum": "old",
            }
        ]
        updated = [{**existing[0], "checksum": "new"}]
        completed = subprocess.CompletedProcess([], 0, "", "")

        with patch.object(update_catalog.subprocess, "run", return_value=completed):
            with self.assertRaisesRegex(SystemExit, "bump the plugin version"):
                update_catalog.reject_replaced_versions(existing, updated)

    def test_accepts_same_checksum_for_existing_version(self):
        entry = {
            "id": "office-universal",
            "version": "1.0.0",
            "abi": "arm64-v8a",
            "checksum": "same",
        }
        completed = subprocess.CompletedProcess([], 0, "", "")

        with patch.object(update_catalog.subprocess, "run", return_value=completed):
            update_catalog.reject_replaced_versions([entry], [entry.copy()])

    def test_rejects_checksum_found_only_in_git_history(self):
        updated = [
            {
                "id": "office-universal",
                "version": "0.9.0",
                "abi": "x86_64",
                "checksum": "replacement",
            }
        ]
        history = subprocess.CompletedProcess([], 0, "abc123\n", "")
        snapshot = subprocess.CompletedProcess(
            [],
            0,
            json.dumps(
                {
                    "plugins": [
                        {
                            "id": "office-universal",
                            "version": "0.9.0",
                            "abi": "x86_64",
                            "checksum": "published",
                        }
                    ]
                }
            ),
            "",
        )

        with patch.object(update_catalog.subprocess, "run", side_effect=[history, snapshot]):
            with self.assertRaisesRegex(SystemExit, "bump the plugin version"):
                update_catalog.reject_replaced_versions([], updated)


class PackageValidatorTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)
        self.validator = SCRIPTS / "validate-plugin-package.py"
        self.manifest = {
            "id": "office-universal",
            "name": "Office Universal",
            "version": "1.2.3",
            "description": "test fixture",
            "minAppVersion": 1,
            "entryClass": "ai.viewit.plugins.officeuniversal.OfficeUniversalPlugin",
            "supportedFormats": ["docx"],
            "capabilities": ["document-render"],
            "base": "view",
            "runtime": "native",
            "abi": "arm64-v8a",
            "abiVersion": 1,
            "downloadUrl": "https://example.test/office-universal.zip",
            "sizeBytes": 1,
            "installedSizeBytes": 1,
            "checksum": "0" * 64,
            "jsEntry": "index.js",
        }
        self.required = [
            ("dex/classes.dex", b"dex"),
            ("index.js", b"js"),
            ("lib/arm64-v8a/libviewit_plugin_office_universal.so", b"so"),
        ]

    def tearDown(self):
        self.temporary.cleanup()

    def make_package(self, name: str, entries=None) -> Path:
        path = self.directory / name
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            with zipfile.ZipFile(path, "w") as archive:
                archive.writestr("plugin.json", json.dumps(self.manifest))
                for entry, data in entries if entries is not None else self.required:
                    archive.writestr(entry, data)
        return path

    def validate(self, package: Path, **overrides):
        command = [
            sys.executable,
            str(self.validator),
            str(package),
            "--id",
            overrides.get("plugin_id", "office-universal"),
            "--version",
            overrides.get("version", "1.2.3"),
            "--abi",
            overrides.get("abi", "arm64-v8a"),
        ]
        return subprocess.run(command, cwd=ROOT, capture_output=True, text=True)

    def test_accepts_canonical_package(self):
        result = self.validate(self.make_package("valid.zip"))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_missing_required_entry(self):
        result = self.validate(self.make_package("missing.zip", self.required[1:]))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing required entries", result.stderr)

    def test_rejects_empty_required_entry(self):
        entries = [(name, b"" if name == "dex/classes.dex" else data) for name, data in self.required]
        result = self.validate(self.make_package("empty.zip", entries))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("empty required entry", result.stderr)

    def test_rejects_duplicate_entry(self):
        entries = self.required + [("dex/classes.dex", b"other")]
        result = self.validate(self.make_package("duplicate.zip", entries))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("duplicate entries", result.stderr)

    def test_rejects_path_traversal(self):
        result = self.validate(self.make_package("traversal.zip", self.required + [("../escape", b"x")]))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unsafe entries", result.stderr)

    def test_rejects_wrong_version_or_abi(self):
        package = self.make_package("identity.zip")
        self.assertNotEqual(self.validate(package, version="9.9.9").returncode, 0)
        self.assertNotEqual(self.validate(package, abi="x86_64").returncode, 0)

    def test_rejects_extra_native_library(self):
        entries = self.required + [("lib/x86_64/libviewit_plugin_office_universal.so", b"other")]
        result = self.validate(self.make_package("extra-abi.zip", entries))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unexpected native libraries", result.stderr)


class CatalogStagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.artifacts = self.root / "artifacts"
        self.destination = self.root / "staged"
        self.artifacts.mkdir()
        self.payload = b"plugin-data"
        self.package = self.artifacts / "plugin.zip"
        self.package.write_bytes(self.payload)
        digest = __import__("hashlib").sha256(self.payload).hexdigest()
        self.catalog = {
            "plugins": [
                {
                    "id": "fixture",
                    "version": "1",
                    "downloadUrl": "https://invalid.example/plugin.zip",
                    "sizeBytes": len(self.payload),
                    "checksum": digest,
                }
            ]
        }
        self.catalog_path = self.root / "catalog.json"
        self.signed_path = self.root / "catalog.signed.json"
        self.catalog_path.write_text(json.dumps(self.catalog))
        self.signed_path.write_text(json.dumps({"catalog": self.catalog, "signature": "fixture"}))

    def tearDown(self):
        self.temporary.cleanup()

    def test_stages_matching_local_artifact_without_network(self):
        with patch.object(stage_plugin_catalog.urllib.request, "urlopen") as urlopen:
            count = stage_plugin_catalog.stage_artifacts(
                self.catalog_path,
                self.signed_path,
                self.destination,
                [self.artifacts],
                False,
            )

        self.assertEqual(count, 1)
        self.assertEqual((self.destination / "plugin.zip").read_bytes(), self.payload)
        urlopen.assert_not_called()

    def test_rejects_signed_catalog_mismatch(self):
        self.signed_path.write_text(json.dumps({"catalog": {"plugins": []}, "signature": "fixture"}))
        with self.assertRaisesRegex(ValueError, "signed/unsigned catalogs differ"):
            stage_plugin_catalog.stage_artifacts(
                self.catalog_path,
                self.signed_path,
                self.destination,
                [self.artifacts],
                False,
            )

    def test_rejects_missing_or_tampered_local_artifact_without_downloads(self):
        self.package.write_bytes(b"tampered")
        with self.assertRaisesRegex(FileNotFoundError, "no matching local artifact"):
            stage_plugin_catalog.stage_artifacts(
                self.catalog_path,
                self.signed_path,
                self.destination,
                [self.artifacts],
                False,
            )

    def test_downloads_and_validates_missing_artifact(self):
        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self):
                return self.payload

        response = Response()
        response.payload = self.payload
        self.package.unlink()
        with patch.object(stage_plugin_catalog.urllib.request, "urlopen", return_value=response):
            count = stage_plugin_catalog.stage_artifacts(
                self.catalog_path,
                self.signed_path,
                self.destination,
                [self.artifacts],
                True,
            )
        self.assertEqual(count, 1)
        self.assertEqual((self.destination / "plugin.zip").read_bytes(), self.payload)

    def test_rejects_tampered_download(self):
        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self):
                return b"tampered"

        self.package.unlink()
        with patch.object(stage_plugin_catalog.urllib.request, "urlopen", return_value=Response()):
            with self.assertRaisesRegex(ValueError, "staged artifact mismatch"):
                stage_plugin_catalog.stage_artifacts(
                    self.catalog_path,
                    self.signed_path,
                    self.destination,
                    [self.artifacts],
                    True,
                )


class CatalogArtifactVerificationTests(unittest.TestCase):
    def test_verifies_exact_download_filename_not_any_matching_checksum(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payload = b"same"
            digest = __import__("hashlib").sha256(payload).hexdigest()
            (root / "wrong.zip").write_bytes(payload)
            catalog = root / "catalog.json"
            catalog.write_text(
                json.dumps(
                    {
                        "plugins": [
                            {
                                "id": "fixture",
                                "abi": "arm64-v8a",
                                "downloadUrl": "https://example.test/right.zip",
                                "sizeBytes": len(payload),
                                "checksum": digest,
                            }
                        ]
                    }
                )
            )

            with redirect_stdout(StringIO()):
                failures = verify_catalog_artifacts.verify_local_artifacts(
                    catalog, [root], {"fixture"}
                )

            self.assertEqual(failures, 1)

    def test_rejects_checksum_mismatch_for_exact_artifact(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "plugin.zip").write_bytes(b"tampered")
            catalog = root / "catalog.json"
            catalog.write_text(
                json.dumps(
                    {
                        "plugins": [
                            {
                                "id": "fixture",
                                "abi": "arm64-v8a",
                                "downloadUrl": "https://example.test/plugin.zip",
                                "sizeBytes": 4,
                                "checksum": "0" * 64,
                            }
                        ]
                    }
                )
            )
            with redirect_stdout(StringIO()):
                failures = verify_catalog_artifacts.verify_local_artifacts(
                    catalog, [root], {"fixture"}
                )
            self.assertEqual(failures, 1)

    def test_rejects_required_plugin_missing_from_catalog(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog = Path(directory) / "catalog.json"
            catalog.write_text(json.dumps({"plugins": []}))
            with redirect_stdout(StringIO()):
                failures = verify_catalog_artifacts.verify_local_artifacts(
                    catalog, [Path(directory)], {"office-universal"}
                )
            self.assertEqual(failures, 1)

    def test_require_all_rejects_missing_artifact(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            catalog = root / "catalog.json"
            catalog.write_text(
                json.dumps(
                    {
                        "plugins": [
                            {
                                "id": "fixture",
                                "abi": "x86_64",
                                "downloadUrl": "https://example.test/missing.zip",
                                "sizeBytes": 1,
                                "checksum": "0" * 64,
                            }
                        ]
                    }
                )
            )
            with redirect_stdout(StringIO()):
                failures = verify_catalog_artifacts.verify_local_artifacts(
                    catalog, [root], set(), True
                )
            self.assertEqual(failures, 1)


class ShellContractTests(unittest.TestCase):
    def run_shell(self, *args: str):
        return subprocess.run(args, cwd=ROOT, capture_output=True, text=True)

    def test_package_script_rejects_unknown_and_extra_arguments_before_build(self):
        unknown = self.run_shell("bash", "scripts/package-plugins.sh", "--plugin", "unknown")
        extra = self.run_shell(
            "bash", "scripts/package-plugins.sh", "--plugin", "office-universal", "extra"
        )
        self.assertEqual(unknown.returncode, 2)
        self.assertIn("unknown plugin", unknown.stderr)
        self.assertEqual(extra.returncode, 2)
        self.assertIn("usage:", extra.stderr)

    def test_publish_script_rejects_unknown_arguments_without_deploying(self):
        result = self.run_shell("bash", "scripts/publish-plugins.sh", "--not-a-real-option")
        self.assertEqual(result.returncode, 2)
        self.assertIn("usage:", result.stderr)

    def test_office_release_requires_clean_tree_by_default(self):
        with tempfile.TemporaryDirectory(dir=ROOT / "build") as directory:
            checkout = Path(directory) / "repo"
            checkout.mkdir()
            subprocess.run(["git", "init", "-q"], cwd=checkout, check=True)
            scripts = checkout / "scripts"
            scripts.mkdir()
            source = SCRIPTS / "release-office-plugin.sh"
            target = scripts / source.name
            target.write_text(source.read_text())
            target.chmod(0o755)
            (checkout / "dirty.txt").write_text("dirty")

            result = subprocess.run(
                ["bash", str(target), "--skip-apk"],
                cwd=checkout,
                capture_output=True,
                text=True,
            )

        self.assertEqual(result.returncode, 2)
        self.assertIn("worktree must be clean", result.stderr)

    def test_quality_gate_clean_check_includes_untracked_files(self):
        source = (SCRIPTS / "quality-gate.sh").read_text()
        self.assertIn("git status --porcelain --untracked-files=normal", source)

    def test_publish_stage_only_never_invokes_wrangler_or_persists_catalogs(self):
        with tempfile.TemporaryDirectory(dir=TEST_TMP) as directory:
            root = Path(directory)
            scripts = root / "scripts"
            scripts.mkdir()
            publish = scripts / "publish-plugins.sh"
            publish.write_text((SCRIPTS / "publish-plugins.sh").read_text())
            publish.chmod(0o755)
            (root / "plugins").mkdir()
            tracked_catalog = root / "plugins/catalog.json"
            tracked_signed = root / "plugins/catalog.signed.json"
            tracked_catalog.write_text("original catalog")
            tracked_signed.write_text("original signed")

            (scripts / "update-catalog.py").write_text(
                """import json,sys\nfrom pathlib import Path\nout=Path(sys.argv[sys.argv.index('--output')+1])\nout.write_text(json.dumps({'plugins': []}))\n"""
            )
            (scripts / "sign-catalog.py").write_text(
                """import json,sys\nfrom pathlib import Path\nif sys.argv[1]=='sign':\n i=Path(sys.argv[sys.argv.index('--input')+1]); o=Path(sys.argv[sys.argv.index('--output')+1]); o.write_text(json.dumps({'catalog':json.loads(i.read_text()),'signature':'x'}))\n"""
            )
            (scripts / "stage_plugin_catalog.py").write_text(
                """import sys\nprint('[publish] staged and verified 0 catalog artifacts')\n"""
            )
            wrangler = root / "wrangler-must-not-run"
            wrangler.write_text("#!/bin/sh\nexit 99\n")
            wrangler.chmod(0o755)

            result = subprocess.run(
                ["bash", str(publish), "--skip-package", "--stage-only", "--only", "office-universal"],
                cwd=root,
                env={**os.environ, "WRANGLER": str(wrangler)},
                capture_output=True,
                text=True,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(tracked_catalog.read_text(), "original catalog")
            self.assertEqual(tracked_signed.read_text(), "original signed")


if __name__ == "__main__":
    unittest.main()
