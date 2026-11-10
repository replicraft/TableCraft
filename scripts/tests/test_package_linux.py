"""Preparation/refusal contracts, not simulated native Linux qualification."""
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zlib

PACKAGING = Path(__file__).resolve().parents[1] / "packaging"
sys.path.insert(0, str(PACKAGING))
import installer_contract as installers
import package_linux as linux
import release_contract as contract
from test_package_cli import native_binary

TARGET = "x86_64-unknown-linux-gnu"


def application_png():
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

    header = struct.pack(">IIBBBBB", 256, 256, 8, 6, 0, 0, 0)
    pixels = (b"\0" + b"\xff\0\0\xff" * 256) * 256
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
            + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b""))


@unittest.skipUnless(os.name == "posix", "Linux preparation uses POSIX symlinks and shell scripts")
class LinuxPackagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tablecraft-linux-contract-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.0"\n')
        notes = self.root / "docs/releases"
        notes.mkdir(parents=True)
        (notes / "v0.1.0.md").write_text("First-party test release, not a native artifact.\n")
        (self.root / "LICENSE").write_text("First-party test license.\n")
        self.binary = self.root / "tablecraft-desktop"
        self.binary.write_bytes(native_binary(TARGET))
        self.notices = self.root / "notices.txt"
        self.notices.write_text("First-party test notices, not a legal audit.\n")
        self.icon = self.root / "application-256.png"
        self.icon.write_bytes(application_png())
        self.inputs = linux.Inputs("0.1.0", TARGET, self.binary, self.notices, self.icon)
        self.output = self.root / "dist"
        self.addCleanup(patch.stopall)
        patch.object(contract, "ROOT", self.root).start()

    def native_host(self):
        system = patch.object(linux.platform, "system", return_value="Linux")
        machine = patch.object(linux.platform, "machine", return_value="x86_64")
        system.start()
        machine.start()

    def test_wrong_host_or_architecture_refuses_before_tools_and_output(self):
        for system, machine in (("Darwin", "arm64"), ("Linux", "aarch64")):
            with self.subTest(system=system, machine=machine):
                with patch.object(linux.platform, "system", return_value=system), patch.object(linux.platform, "machine", return_value=machine), patch.object(linux, "run") as run:
                    with self.assertRaisesRegex(ValueError, "matching native Linux"):
                        linux.package(self.inputs, self.output, ("deb",))
                    run.assert_not_called()
                self.assertFalse(self.output.exists())

    def test_staging_preserves_payload_identity_and_installs_nondefault_file_handlers(self):
        root = self.root / "root"
        root.mkdir()
        payload = linux.stage(root, self.inputs)
        manifest = json.loads((payload / "RELEASE.json").read_bytes())
        self.assertEqual(manifest["product"], "tablecraft-desktop")
        self.assertEqual(manifest["version"], "0.1.0")
        self.assertEqual(manifest["target"], "x86_64-unknown-linux-gnu")
        self.assertEqual(manifest["files"]["tablecraft-desktop"], hashlib.sha256(self.binary.read_bytes()).hexdigest())
        self.assertEqual((root / "usr/bin/tablecraft-desktop").readlink(), Path("../lib/tablecraft/tablecraft-desktop"))
        desktop = (root / "usr/share/applications/org.replicraft.tablecraft.desktop").read_text()
        self.assertIn('Exec="/usr/bin/tablecraft-desktop" -- %f\n', desktop)
        self.assertIn("MimeType=application/vnd.ms-access;\n", desktop)
        self.assertIn("Icon=org.replicraft.tablecraft\n", desktop)
        mime = (root / "usr/share/mime/packages/org.replicraft.tablecraft.xml").read_text()
        self.assertIn('pattern="*.mdb"', mime)
        self.assertIn('pattern="*.accdb"', mime)
        self.assertEqual(self.binary.read_bytes(), native_binary(TARGET))

    def test_appdir_uses_relocatable_desktop_command_and_adjacent_release_manifest(self):
        root = self.root / "AppDir"
        root.mkdir()
        payload = linux.stage(root, self.inputs, appimage=True)
        self.assertEqual(payload, root / "usr/bin")
        self.assertTrue((payload / "RELEASE.json").is_file())
        self.assertFalse((payload / "tablecraft-desktop").is_symlink())
        entry = (root / "usr/share/applications/org.replicraft.tablecraft.desktop").read_text()
        self.assertIn("Exec=tablecraft-desktop -- %f\n", entry)
        self.assertNotIn('Exec="/usr/bin/', entry)

    def test_invalid_formats_or_missing_appimage_inputs_refuse_before_tools(self):
        self.native_host()
        for formats in ((), ("deb", "deb"), ("msi",), ("appimage",)):
            with self.subTest(formats=formats), patch.object(linux, "run") as run:
                with self.assertRaises(ValueError):
                    linux.package(self.inputs, self.output, formats)
                run.assert_not_called()
                self.assertFalse(self.output.exists())

    def test_existing_outputs_are_preserved_without_running_tools(self):
        self.native_host()
        self.output.mkdir()
        archive = self.output / installers.name("0.1.0", TARGET, "deb")
        archive.write_bytes(b"original output")
        with patch.object(linux, "run") as run:
            with self.assertRaisesRegex(ValueError, "fresh"):
                linux.package(self.inputs, self.output, ("deb",))
            run.assert_not_called()
        self.assertEqual(archive.read_bytes(), b"original output")

    def test_tool_failure_and_partial_checksum_remove_only_owned_outputs(self):
        self.native_host()
        with patch.object(linux, "build_deb", side_effect=linux.ToolFailure("independent tool failure")):
            with self.assertRaises(linux.ToolFailure):
                linux.package(self.inputs, self.output, ("deb",))
        self.assertEqual(list(self.output.iterdir()), [])

        def framed_package(inputs, root, output, temporary):
            # Only the publication failure path uses this synthetic framing.
            # It does not stand in for DEB construction or payload qualification.
            output.write_bytes(b"!<arch>\nfirst-party framing")

        def fail_checksum(path):
            path.with_name(path.name + ".sha256").write_bytes(b"partial")
            raise OSError("independent checksum failure")

        with patch.object(linux, "build_deb", side_effect=framed_package), patch.object(installers, "write_checksum", side_effect=fail_checksum):
            with self.assertRaises(OSError):
                linux.package(self.inputs, self.output, ("deb",))
        self.assertEqual(list(self.output.iterdir()), [])
        self.assertEqual(self.binary.read_bytes(), native_binary(TARGET))

    def test_inventory_rejects_links_outside_its_owned_root(self):
        root = self.root / "root"
        root.mkdir()
        (root / "escaped").symlink_to(self.binary)
        with self.assertRaisesRegex(ValueError, "escapes"):
            linux.inventory(root)

    def test_debian_dependencies_combine_native_symbol_results_and_dlopen_requirements(self):
        temporary = self.root / "dependency-work"
        temporary.mkdir()
        with patch.object(linux, "run", return_value="shlibs:Depends=libc6 (>= 2.35), libgcc-s1 (>= 3.0)\n"):
            dependencies = linux.debian_dependencies(self.inputs, temporary)
        self.assertIn("libc6 (>= 2.35)", dependencies)
        self.assertIn("libfontconfig1", dependencies)
        self.assertIn("libxkbcommon0", dependencies)
        self.assertIn("libegl1", dependencies)
        self.assertIn("libgl1", dependencies)

    def test_rpm_inventory_rejects_changed_payload_hash(self):
        root = self.root / "root"
        root.mkdir()
        linux.stage(root, self.inputs)
        changed = "/usr/lib/tablecraft/tablecraft-desktop\t33261\t\t" + "0" * 64 + "\n"
        with patch.object(linux, "run", return_value=changed):
            with self.assertRaisesRegex(ValueError, "SHA-256 differs"):
                linux.verify_rpm_files(self.root / "unexecuted.rpm", root)

    def test_deb_stream_rejects_unlisted_files_without_extracting_them(self):
        root = self.root / "root"
        root.mkdir()
        linux.stage(root, self.inputs)
        raw = io.BytesIO()
        with tarfile.open(fileobj=raw, mode="w") as stream:
            member = tarfile.TarInfo("../../unowned")
            member.size = 1
            stream.addfile(member, io.BytesIO(b"x"))

        class Process:
            def __init__(self):
                self.stdout = io.BytesIO(raw.getvalue())

            def poll(self):
                return 0

            def wait(self, timeout=None):
                return 0

        with patch.object(linux.subprocess, "Popen", return_value=Process()):
            with self.assertRaisesRegex(ValueError, "membership"):
                linux.verify_deb_files(self.root / "unexecuted.deb", root)
        self.assertFalse((self.root / "unowned").exists())

    def test_apprun_keeps_literal_filenames_and_runs_without_changing_host_defaults(self):
        root = self.root / "AppDir with spaces"
        binary = root / "usr/bin/tablecraft-desktop"
        binary.parent.mkdir(parents=True)
        binary.write_text('#!/bin/sh\nprintf "%s\\n" "$@"\n')
        binary.chmod(0o755)
        launcher = root / "AppRun"
        launcher.write_bytes((linux.ASSETS / "AppRun").read_bytes())
        launcher.chmod(0o755)
        environment = os.environ.copy()
        environment.pop("APPDIR", None)
        result = subprocess.run([launcher, "--", "folder with spaces/日本語.accdb"], capture_output=True, text=True, check=True, env=environment)
        self.assertEqual(result.stdout.splitlines(), ["--", "folder with spaces/日本語.accdb"])

    def test_native_tools_do_not_inherit_release_signing_secrets(self):
        with patch.dict(os.environ, {
            "TABLECRAFT_UPDATE_SIGNING_KEY": "first-party-test-key",
            "TABLECRAFT_UPDATE_SIGNING_PASSWORD": "first-party-test-password",
        }):
            output = linux.run([sys.executable, "-c", """
import os
import sys
assert "TABLECRAFT_UPDATE_SIGNING_KEY" not in os.environ
assert "TABLECRAFT_UPDATE_SIGNING_PASSWORD" not in os.environ
assert sys.stdin.read() == ""
print("clean")
"""])
        self.assertEqual(output, "clean\n")

    def test_prerelease_version_sorts_before_its_final_native_package(self):
        self.assertEqual(linux.package_version("0.1.0"), "0.1.0")
        self.assertEqual(linux.package_version("0.1.0-rc.1"), "0.1.0~rc.1")


if __name__ == "__main__":
    unittest.main()
