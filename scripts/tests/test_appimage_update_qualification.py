"""Harness safety/refusal checks; these do not claim native AppImage execution."""
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import appimage_update_qualification as native


class AppImageQualificationTests(unittest.TestCase):
    def test_child_environment_excludes_production_secrets_and_old_runtime_identity(self):
        with patch.dict(os.environ, {
            "UPDATE_SIGNING_KEY": "secret", "UPDATE_SIGNING_PASSWORD": "password",
            "TABLECRAFT_UPDATE_SIGNING_KEY": "secret", "TABLECRAFT_UPDATE_SIGNING_PASSWORD": "password",
            "APPDIR": "/stale", "APPIMAGE": "/stale-image", "ARGV0": "stale", "OWD": "/stale-cwd",
            "PATH": "/known-tools",
        }, clear=True):
            self.assertEqual(native.environment(), {"PATH": "/known-tools"})

    def test_receiver_must_be_new_headless_image_at_the_original_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            image = root / "Original.AppImage"
            path = root / "receipt.txt"
            valid = (f"receiver=tablecraft-headless-update-receiver\nversion=0.1.1\npid=42\n"
                     f"APPIMAGE={image}\nAPPDIR=/new-runtime\nARGV0={image}\nOWD={root}\n")
            path.write_text(valid)
            fields = native.receiver_receipt(path, "0.1.1", image, Path("/old-runtime"))
            self.assertEqual(fields["APPDIR"], "/new-runtime")
            for changed in (
                valid.replace("headless-update-receiver", "desktop"),
                valid.replace("version=0.1.1", "version=0.1.0"),
                valid.replace(str(image), "/unrelated.AppImage"),
                valid.replace("APPDIR=/new-runtime", "APPDIR=/old-runtime"),
                valid + "APPDIR=/duplicate\n",
                valid.replace(f"ARGV0={image}", "ARGV0=stale-appimage-argument"),
            ):
                path.write_text(changed)
                with self.subTest(receipt=changed):
                    with self.assertRaises(ValueError):
                        native.receiver_receipt(path, "0.1.1", image, Path("/old-runtime"))

    def test_process_observation_distinguishes_finished_helpers_without_signalling_them(self):
        for state, expected in (("S", True), ("Z", False)):
            with patch.object(Path, "read_text", return_value=f"123 (name with ) spaces) {state} 1 2 3"):
                self.assertEqual(native.process_alive(123), expected)
        with patch.object(Path, "read_text", side_effect=FileNotFoundError):
            self.assertFalse(native.process_alive(123))

    def test_wrong_native_host_refuses_before_creating_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            with patch.object(native.contract, "validate_version"), patch.object(native.platform, "system", return_value="Darwin"):
                with self.assertRaisesRegex(ValueError, "matching native Linux"):
                    native.qualify(root / "release.AppImage", "0.1.0", "x86_64-unknown-linux-gnu",
                                   root / "minisign", root / "appimagetool", root / "runtime", evidence)
            self.assertFalse(evidence.exists())


if __name__ == "__main__":
    unittest.main()
