"""The signing bootstrap must authenticate before creating executable output."""
import hashlib
import io
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import minisign_tool as tool
from test_package_cli import native_binary


def archive(binary, *, symlink=False, architecture="x86_64"):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as stream:
        entry = tarfile.TarInfo(f"minisign-linux/{architecture}/minisign")
        entry.mode = 0o755
        if symlink:
            entry.type = tarfile.SYMTYPE
            entry.linkname = "/tmp/foreign"
            stream.addfile(entry)
        else:
            entry.size = len(binary)
            stream.addfile(entry, io.BytesIO(binary))
    return output.getvalue()


class MinisignBootstrapTests(unittest.TestCase):
    def test_changed_digest_creates_nothing(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "tool"
            with self.assertRaisesRegex(ValueError, "digest differs"):
                tool.install(b"changed upstream bytes", output)
            self.assertFalse(output.exists())

    def test_verified_member_is_extracted_exclusively_with_executable_permissions(self):
        data = archive(native_binary("x86_64-unknown-linux-gnu"))
        with tempfile.TemporaryDirectory() as temporary, patch.object(tool, "SHA256", hashlib.sha256(data).hexdigest()):
            output = Path(temporary) / "tool"
            binary = tool.install(data, output)
            self.assertEqual(binary.read_bytes(), native_binary("x86_64-unknown-linux-gnu"))
            self.assertEqual(binary.stat().st_mode & 0o777, 0o755)
            with self.assertRaises(FileExistsError):
                tool.install(data, output)
            self.assertEqual(binary.read_bytes(), native_binary("x86_64-unknown-linux-gnu"))

    def test_arm64_member_uses_native_elf_admission_and_unknown_hosts_create_nothing(self):
        payload = native_binary("aarch64-unknown-linux-gnu")
        data = archive(payload, architecture="aarch64")
        with tempfile.TemporaryDirectory() as temporary, patch.object(tool, "SHA256", hashlib.sha256(data).hexdigest()):
            output = Path(temporary) / "arm64-tool"
            binary = tool.install(data, output, "aarch64")
            self.assertEqual(binary.read_bytes(), payload)
            for architecture in ("x86_64", "riscv64"):
                refused = Path(temporary) / architecture
                with self.assertRaises(ValueError):
                    tool.install(data, refused, architecture)
                self.assertFalse(refused.exists())

    def test_links_and_wrong_architecture_refuse_before_output(self):
        for data in (archive(b"", symlink=True), archive(native_binary("aarch64-unknown-linux-gnu"))):
            with tempfile.TemporaryDirectory() as temporary, patch.object(tool, "SHA256", hashlib.sha256(data).hexdigest()):
                output = Path(temporary) / "tool"
                with self.assertRaises(ValueError):
                    tool.install(data, output)
                self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
