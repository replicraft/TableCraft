"""Exercise the real signature tool against tampering and release identity changes."""
import base64
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import update_signatures as signatures


@unittest.skipUnless(shutil.which("minisign"), "Minisign is required for cryptographic tests")
class UpdateSignatureTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tablecraft-sign-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.public = self.root / "TEST-ONLY.pub"
        self.secret = self.root / "TEST-ONLY.key"
        subprocess.run(["minisign", "-G", "-W", "-p", str(self.public),
                        "-s", str(self.secret)], capture_output=True, check=True)
        self.assets = self.root / "assets"
        self.assets.mkdir()
        for name in signatures.names("0.1.0", "desktop"):
            (self.assets / name).write_bytes(b"independent signed payload: " + name.encode())
        self.output = self.root / "signatures"
        self.environment = patch.dict(os.environ, {
            "TABLECRAFT_UPDATE_SIGNING_KEY": self.secret.read_text(),
            "TABLECRAFT_UPDATE_SIGNING_PASSWORD": "",
        })
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def run_signatures(self, *, signing=False, dry_run=True):
        signatures.run("0.1.0", "desktop", self.assets, self.output,
                       self.public, "minisign", signing=signing, dry_run=dry_run)

    def test_complete_set_verifies_and_payload_tampering_refuses(self):
        self.run_signatures(signing=True)
        self.run_signatures()
        archive = next(self.assets.iterdir())
        archive.write_bytes(archive.read_bytes() + b"changed")
        with self.assertRaises(ValueError):
            self.run_signatures()

    def test_valid_signature_for_wrong_version_or_filename_refuses(self):
        self.run_signatures(signing=True)
        archive = next(self.assets.iterdir())
        signature = self.output / (archive.name + ".minisig")
        subprocess.run(["minisign", "-S", "-s", str(self.secret), "-m", str(archive),
                        "-x", str(signature), "-t", "tablecraft-update 0.0.9 other.zip"],
                       capture_output=True, check=True)
        with self.assertRaisesRegex(ValueError, "not bound"):
            self.run_signatures()

    def test_wrong_key_and_missing_signature_refuse(self):
        self.run_signatures(signing=True)
        self.public.unlink()
        subprocess.run(["minisign", "-G", "-W", "-p", str(self.public),
                        "-s", str(self.root / "other.key")],
                       capture_output=True, check=True)
        with self.assertRaises(ValueError):
            self.run_signatures()
        next(self.output.iterdir()).unlink()
        with self.assertRaisesRegex(ValueError, "complete archive set"):
            self.run_signatures()

    def test_failed_signing_cleans_owned_outputs_and_preserves_archives(self):
        before = {p.name: p.read_bytes() for p in self.assets.iterdir()}
        with patch.dict(os.environ, {"TABLECRAFT_UPDATE_SIGNING_KEY": "invalid"}):
            with self.assertRaises(ValueError):
                self.run_signatures(signing=True)
        self.assertFalse(self.output.exists())
        self.assertEqual(before, {p.name: p.read_bytes() for p in self.assets.iterdir()})

    def test_minisign_child_has_no_secret_environment_or_interactive_input(self):
        signatures.execute([sys.executable, "-c", """
import os
import sys

assert "TABLECRAFT_UPDATE_SIGNING_KEY" not in os.environ
assert "TABLECRAFT_UPDATE_SIGNING_PASSWORD" not in os.environ
assert sys.stdin.read() == ""
"""])
        self.assertEqual(os.environ["TABLECRAFT_UPDATE_SIGNING_KEY"], self.secret.read_text())

    def test_password_uses_stdin_without_exposing_either_secret_in_child_environment(self):
        password = "first-party-test-password"
        expected = hashlib.sha256((password + "\n").encode("utf-8")).hexdigest()
        program = """
import hashlib
import os
import sys

assert "TABLECRAFT_UPDATE_SIGNING_KEY" not in os.environ
assert "TABLECRAFT_UPDATE_SIGNING_PASSWORD" not in os.environ
assert hashlib.sha256(sys.stdin.buffer.read()).hexdigest() == sys.argv[1]
"""
        with patch.dict(os.environ, {"TABLECRAFT_UPDATE_SIGNING_PASSWORD": password}):
            signatures.execute([sys.executable, "-c", program, expected], password=password)
            self.assertEqual(os.environ["TABLECRAFT_UPDATE_SIGNING_PASSWORD"], password)

    def test_encrypted_key_signs_and_wrong_or_missing_password_cleans_outputs(self):
        password = "independent-test-key-password"
        encrypted_public = self.root / "encrypted.pub"
        encrypted_secret = self.root / "encrypted.key"
        subprocess.run(
            ["minisign", "-G", "-p", str(encrypted_public), "-s", str(encrypted_secret)],
            input=(password + "\n" + password + "\n").encode("utf-8"),
            capture_output=True, check=True, timeout=120,
        )
        self.public = encrypted_public
        environment = {
            "TABLECRAFT_UPDATE_SIGNING_KEY": encrypted_secret.read_text(),
            "TABLECRAFT_UPDATE_SIGNING_PASSWORD": password,
        }
        with patch.dict(os.environ, environment):
            self.run_signatures(signing=True)
            # Verification has no private key or password input at all.
            with patch.dict(os.environ, {
                "TABLECRAFT_UPDATE_SIGNING_KEY": "",
                "TABLECRAFT_UPDATE_SIGNING_PASSWORD": "",
            }):
                self.run_signatures()
        shutil.rmtree(self.output)
        original_archives = {path.name: path.read_bytes() for path in self.assets.iterdir()}
        for supplied in ("wrong-test-password", ""):
            environment["TABLECRAFT_UPDATE_SIGNING_PASSWORD"] = supplied
            with self.subTest(password_present=bool(supplied)):
                with patch.dict(os.environ, environment):
                    with self.assertRaisesRegex(ValueError, "Minisign operation failed") as failed:
                        self.run_signatures(signing=True)
                self.assertNotIn(password, str(failed.exception))
                self.assertNotIn(environment["TABLECRAFT_UPDATE_SIGNING_KEY"], str(failed.exception))
                self.assertFalse(self.output.exists())
                self.assertEqual(
                    original_archives,
                    {path.name: path.read_bytes() for path in self.assets.iterdir()},
                )

    def test_invalid_password_input_refuses_before_starting_minisign(self):
        for password in ("two\nlines", "trailing\r", "x" * 4097):
            with self.subTest(length=len(password)):
                with patch.dict(os.environ, {"TABLECRAFT_UPDATE_SIGNING_PASSWORD": password}):
                    with patch.object(signatures, "execute") as execute:
                        with self.assertRaisesRegex(ValueError, "Signing password"):
                            self.run_signatures(signing=True)
                        execute.assert_not_called()
                self.assertFalse(self.output.exists())

    def test_production_refuses_alternative_and_test_labeled_pinned_keys(self):
        real_text = self.public.read_text()
        with patch.object(signatures, "PUBLIC_KEY", self.public):
            signatures.check_key(self.public, production=True)
            alternative = self.root / "alternative.pub"
            subprocess.run(["minisign", "-G", "-W", "-p", str(alternative),
                            "-s", str(self.root / "alternative.key")],
                           capture_output=True, check=True)
            with self.assertRaisesRegex(ValueError, "pinned"):
                signatures.check_key(alternative, production=True)
            for marker in ("TEST ONLY", "TEST-ONLY", "TEST_ONLY"):
                self.public.write_text(
                    f"untrusted comment: {marker} update public key\n"
                    + real_text.splitlines()[1] + "\n"
                )
                with self.subTest(marker=marker):
                    with self.assertRaisesRegex(ValueError, "test-only"):
                        signatures.check_key(self.public, production=True)

    def test_production_refuses_real_fixture_key_after_label_or_key_id_changes(self):
        fixture = signatures.TEST_PUBLIC_KEY.read_text().splitlines()[1]
        changed_id = bytearray(base64.b64decode(fixture))
        changed_id[2] ^= 1
        variants = (fixture, base64.b64encode(changed_id).decode("ascii"))
        with patch.object(signatures, "PUBLIC_KEY", self.public):
            for encoded in variants:
                self.public.write_text(
                    "untrusted comment: TableCraft release public key\n" + encoded + "\n"
                )
                with self.subTest(encoded=encoded):
                    with self.assertRaisesRegex(ValueError, "known test-only"):
                        self.run_signatures(signing=True, dry_run=False)
                self.assertFalse(self.output.exists())

    def test_oversized_signing_key_cleans_output(self):
        with patch.dict(os.environ, {"TABLECRAFT_UPDATE_SIGNING_KEY": "x" * 4097}):
            with self.assertRaisesRegex(ValueError, "key-size"):
                self.run_signatures(signing=True)
        self.assertFalse(self.output.exists())

    def test_production_refuses_missing_pinned_key_and_symlinked_payload(self):
        with patch.object(signatures, "PUBLIC_KEY", self.root / "missing-release-key.pub"):
            with self.assertRaises(ValueError):
                self.run_signatures(signing=True, dry_run=False)
        archive = next(self.assets.iterdir())
        archive.unlink()
        archive.symlink_to(self.secret)
        with self.assertRaises(ValueError):
            self.run_signatures(signing=True)


if __name__ == "__main__":
    unittest.main()
