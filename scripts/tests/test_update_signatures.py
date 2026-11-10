"""Exercise the real signature tool against tampering and release identity changes."""
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
