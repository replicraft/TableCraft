"""Exercise the native request receiver without Finder, Terminal or registration."""
import os
from pathlib import Path
import plistlib
import shlex
import shutil
import subprocess
import tempfile
import unittest

RECEIVER = os.environ.get("TABLECRAFT_TEST_MACOS_RECEIVER")


@unittest.skipUnless(RECEIVER, "Native macOS receiver supplied by macOS release builds")
class MacOSReceiverTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tablecraft-open-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name).resolve()
        self.directory.chmod(0o700)
        app = self.directory / "a b 用户 ' $ ` %" / "TableCraft.app"
        self.receiver = app / "Contents/MacOS/TableCraft"
        self.receiver.parent.mkdir(parents=True)
        shutil.copyfile(RECEIVER, self.receiver)
        self.receiver.chmod(0o755)
        self.cli = app / "Contents/Resources/tablecraft"
        self.cli.parent.mkdir(parents=True)
        self.capture = self.directory / "arguments"
        self.cli.write_text(
            "#!/bin/sh\nprintf '%s\\0' \"$@\" >> " + shlex.quote(str(self.capture)) + "\nexit 2\n"
        )
        self.cli.chmod(0o755)
        self.request = self.directory / "request.plist"
        (self.directory / "Inspect.command").write_text("owned request launcher\n")

    def invoke(self, files, executable=None):
        self.request.write_bytes(plistlib.dumps({
            "executable": str(executable or self.cli), "files": files,
        }))
        self.request.chmod(0o600)
        return subprocess.run(
            [str(self.receiver), "--terminal-request", str(self.request)],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=15,
        )

    def test_database_names_are_literal_arguments_and_nonzero_status_survives(self):
        files = ["/tmp/用户 a ' \" $() `cmd` %1.accdb", "/tmp/-dash.mdb"]
        result = self.invoke(files)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertEqual(self.capture.read_bytes().split(b"\0")[:-1], [
            value.encode() for file in files for value in ["open", "--wait", "--", file]
        ])
        self.assertFalse(self.request.exists())
        self.assertFalse((self.directory / "Inspect.command").exists())

    def test_request_cannot_select_another_executable(self):
        result = self.invoke(["/tmp/example.accdb"], executable="/bin/sh")
        self.assertEqual(result.returncode, 1)
        self.assertFalse(self.capture.exists())

    def test_help_without_documents_does_not_wait_when_redirected(self):
        result = self.invoke([])
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertEqual(self.capture.read_bytes(), b"--help\0")

    def test_request_directory_permissions_are_required(self):
        self.directory.chmod(0o755)
        result = self.invoke(["/tmp/example.accdb"])
        self.assertEqual(result.returncode, 1)
        self.assertFalse(self.capture.exists())


if __name__ == "__main__":
    unittest.main()
