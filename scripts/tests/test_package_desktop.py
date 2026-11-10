"""Portable desktop contracts use independent native headers, never launch a GUI."""
import hashlib
import json
from pathlib import Path
import plistlib
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile
import zlib

PACKAGING = Path(__file__).resolve().parents[1] / "packaging"
sys.path.insert(0, str(PACKAGING))
import package_desktop as desktop
import desktop_release as release
import release_contract as contract
from test_package_cli import VERSION, native_binary


def outline_icon():
    """Independently authored RGBA pixels in a minimal real PNG-backed ICNS."""
    def chunk(kind, body):
        checksum = zlib.crc32(kind + body)
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", checksum)

    header = struct.pack(">IIBBBBB", 16, 16, 8, 6, 0, 0, 0)
    pixels = (b"\0" + b"\xff\x00\x00\xff" * 16) * 16
    png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
           + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b""))
    entry = b"icp4" + struct.pack(">I", len(png) + 8) + png
    return b"icns" + struct.pack(">I", len(entry) + 8) + entry


class DesktopPackagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tablecraft-desktop-contract-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{VERSION}"\n')
        notes = self.root / "docs/releases"
        notes.mkdir(parents=True)
        (notes / f"v{VERSION}.md").write_text("Independent test release.\n")
        (self.root / "LICENSE").write_text("Independent test product license.\n")
        self.notices = self.root / "desktop-notices.txt"
        self.notices.write_text("Synthetic contract notice. Not a distributable license audit.\n")
        self.icon = self.root / "reviewed.icns"
        self.icon.write_bytes(outline_icon())
        self.addCleanup(patch.stopall)
        patch.object(contract, "ROOT", self.root).start()

    def package(self, target="x86_64-pc-windows-msvc"):
        binary = self.root / (target + ".bin")
        binary.write_bytes(native_binary(target))
        icon = self.icon if "apple" in target else None
        return desktop.package(VERSION, target, binary, self.notices, self.root / "dist", icon)

    def rewrite_zip(self, archive, transform):
        with zipfile.ZipFile(archive) as stream:
            entries = [(entry, stream.read(entry)) for entry in stream.infolist()]
        with zipfile.ZipFile(archive, "w") as stream:
            for entry, content in transform(entries):
                stream.writestr(entry, content)

    def test_packaging_command_generates_notices_before_creating_archive(self):
        target = "x86_64-pc-windows-msvc"
        binary = self.root / "desktop.exe"
        binary.write_bytes(native_binary(target))
        arguments = [
            "package_desktop.py", "package", "--version", VERSION,
            "--target", target, "--binary", str(binary),
            "--output", str(self.root / "dist"),
        ]
        generated = b"Generated exact dependency notices.\n"
        with patch.object(sys, "argv", arguments):
            with patch.object(desktop.desktop_notices, "generate", return_value=({}, {}, generated)) as generate:
                desktop.main()
        generate.assert_called_once()
        archive = self.root / "dist" / desktop.archive_name(VERSION, target)
        with zipfile.ZipFile(archive) as stream:
            self.assertEqual(stream.read(desktop.NOTICES), generated)

        # A missing source blocks packaging before it creates any output directory.
        arguments[-1] = str(self.root / "refused")
        with patch.object(sys, "argv", arguments):
            with patch.object(desktop.desktop_notices, "generate", side_effect=ValueError("Missing source")):
                with self.assertRaises(SystemExit):
                    desktop.main()
        self.assertFalse((self.root / "refused").exists())

    def test_all_six_targets_have_closed_payloads_and_real_macos_bundles(self):
        for target in contract.TARGETS:
            with self.subTest(target=target):
                archive = self.package(target)
                icon = self.icon if "apple" in target else None
                desktop.verify_archive(archive, VERSION, target, self.notices, icon)
                checksum = archive.with_name(archive.name + ".sha256").read_text()
                self.assertEqual(checksum, f"{contract.digest(archive)}  {archive.name}\n")
                if "apple" in target:
                    with zipfile.ZipFile(archive) as stream:
                        info = plistlib.loads(stream.read("TableCraft.app/Contents/Info.plist"))
                        self.assertEqual(info["CFBundleIdentifier"], "org.replicraft.tablecraft")
                        self.assertEqual(info["CFBundleExecutable"], "tablecraft-desktop")
                        self.assertNotIn("CFBundleDocumentTypes", info)
                        self.assertNotIn("LSMinimumSystemVersion", info)
                        binary = stream.getinfo("TableCraft.app/Contents/MacOS/tablecraft-desktop")
                        self.assertEqual(stat.S_IMODE(binary.external_attr >> 16), 0o755)
                        self.assertEqual(
                            stream.read("TableCraft.app/Contents/Resources/TableCraft.icns"),
                            self.icon.read_bytes(),
                        )
                with self.assertRaisesRegex(ValueError, "fresh paths"):
                    self.package(target)

    def test_drag_install_preserves_the_macos_release_manifest_and_payload_hashes(self):
        archive = self.package("aarch64-apple-darwin")
        installed = self.root / "Applications" / "TableCraft.app"
        with zipfile.ZipFile(archive) as stream:
            self.assertNotIn("RELEASE.json", stream.namelist())
            for entry in stream.infolist():
                relative = Path(entry.filename).relative_to("TableCraft.app")
                destination = installed / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(stream.read(entry))

        manifest = installed / "Contents/Resources/RELEASE.json"
        self.assertTrue(manifest.is_file())
        self.assertFalse((installed.parent / "RELEASE.json").exists())
        metadata = json.loads(manifest.read_bytes())
        self.assertEqual(metadata["product"], "tablecraft-desktop")
        self.assertEqual(metadata["version"], VERSION)
        self.assertEqual(metadata["target"], "aarch64-apple-darwin")
        for name, digest in metadata["files"].items():
            relative = Path(name).relative_to("TableCraft.app")
            self.assertEqual(hashlib.sha256((installed / relative).read_bytes()).hexdigest(), digest)

    def test_external_root_macos_manifest_is_not_an_installable_package(self):
        target = "aarch64-apple-darwin"
        archive = self.package(target)

        def move_manifest_outside_bundle(entries):
            for index, (entry, content) in enumerate(entries):
                if entry.filename == "TableCraft.app/Contents/Resources/RELEASE.json":
                    moved = zipfile.ZipInfo("RELEASE.json")
                    moved.external_attr = entry.external_attr
                    entries[index] = (moved, content)
            return entries

        self.rewrite_zip(archive, move_manifest_outside_bundle)
        with self.assertRaisesRegex(ValueError, "Unexpected or duplicate"):
            desktop.verify_archive(archive, VERSION, target, self.notices, self.icon)

    def test_reviewed_notices_and_actual_icon_pixels_are_required(self):
        target = "aarch64-apple-darwin"
        binary = self.root / "binary"
        binary.write_bytes(native_binary(target))
        with self.assertRaisesRegex(ValueError, "explicit reviewed"):
            desktop.package(VERSION, target, binary, self.notices, self.root / "dist")
        self.icon.write_bytes(b"icns\0\0\0\x08")
        with self.assertRaisesRegex(ValueError, "real image"):
            desktop.package(VERSION, target, binary, self.notices, self.root / "dist", self.icon)
        self.icon.write_bytes(outline_icon()[:-1] + b"x")
        with self.assertRaisesRegex(ValueError, "checksum"):
            desktop.validate_icon(self.icon)
        self.notices.write_text(" \n")
        with self.assertRaisesRegex(ValueError, "blank"):
            desktop.package(VERSION, target, binary, self.notices, self.root / "dist", self.icon)
        self.assertFalse((self.root / "dist").exists())

    def test_icns_table_of_contents_matches_actual_image_records(self):
        original = outline_icon()
        record = original[8:16]
        table = b"TOC " + struct.pack(">I", 16) + record
        body = table + original[8:]
        self.icon.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)
        desktop.validate_icon(self.icon)
        changed = bytearray(self.icon.read_bytes())
        struct.pack_into(">I", changed, 20, 999)
        self.icon.write_bytes(changed)
        with self.assertRaisesRegex(ValueError, "TOC differs"):
            desktop.validate_icon(self.icon)

    def test_wrong_architecture_and_version_refuse_before_output(self):
        binary = self.root / "binary"
        binary.write_bytes(native_binary("x86_64-unknown-linux-gnu"))
        with self.assertRaises(ValueError):
            desktop.package(VERSION, "aarch64-unknown-linux-gnu", binary, self.notices, self.root / "dist")
        with self.assertRaises(ValueError):
            desktop.package("99.0.0", "x86_64-unknown-linux-gnu", binary, self.notices, self.root / "dist")
        self.assertFalse((self.root / "dist").exists())

    def test_unlisted_paths_duplicates_symlinks_and_wrong_modes_are_refused(self):
        target = "x86_64-pc-windows-msvc"
        archive = self.package(target)
        original = archive.read_bytes()
        outside = self.root / "escaped.txt"
        for name in ("../escaped.txt", str(outside), "extra.dll", "TableCraft.app/../LICENSE"):
            archive.write_bytes(original)
            with zipfile.ZipFile(archive, "a") as stream:
                entry = zipfile.ZipInfo(name)
                entry.external_attr = (stat.S_IFREG | 0o644) << 16
                stream.writestr(entry, b"untrusted")
            with self.assertRaisesRegex(ValueError, "Unexpected or duplicate"):
                desktop.verify_archive(archive, VERSION, target, self.notices)
            self.assertFalse(outside.exists())

        archive.write_bytes(original)
        def duplicate(entries):
            first, content = entries[0]
            repeated = zipfile.ZipInfo(first.filename)
            repeated.external_attr = first.external_attr
            return entries + [(repeated, content)]
        with self.assertWarns(UserWarning):
            self.rewrite_zip(archive, duplicate)
        with self.assertRaisesRegex(ValueError, "duplicate"):
            desktop.verify_archive(archive, VERSION, target, self.notices)
        for kind, mode in [(stat.S_IFLNK, 0o644), (stat.S_IFREG, 0o777)]:
            archive.write_bytes(original)
            def change(entries):
                entries[0][0].external_attr = (kind | mode) << 16
                return entries
            self.rewrite_zip(archive, change)
            with self.assertRaises(ValueError):
                desktop.verify_archive(archive, VERSION, target, self.notices)

    def test_linux_tar_refuses_links_before_extracting(self):
        target = "x86_64-unknown-linux-gnu"
        archive = self.package(target)
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.DIRTYPE):
            with tarfile.open(archive, "w:gz") as stream:
                entry = tarfile.TarInfo("tablecraft-desktop")
                entry.type = kind
                entry.linkname = "/tmp/not-a-release-resource"
                stream.addfile(entry)
            with self.assertRaisesRegex(ValueError, "regular nonsparse"):
                desktop.verify_archive(archive, VERSION, target, self.notices)

    def test_rehashed_notice_replacement_and_duplicate_manifest_keys_are_not_authority(self):
        target = "x86_64-pc-windows-msvc"
        archive = self.package(target)
        original = archive.read_bytes()
        def forged(entries):
            changed = b"Unreviewed replacement attribution."
            for index, (entry, content) in enumerate(entries):
                if entry.filename == desktop.NOTICES:
                    entries[index] = (entry, changed)
                if entry.filename == desktop.MANIFEST:
                    metadata = json.loads(content)
                    metadata["files"][desktop.NOTICES] = hashlib.sha256(changed).hexdigest()
                    entries[index] = (entry, json.dumps(metadata).encode())
            return entries
        self.rewrite_zip(archive, forged)
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            desktop.verify_archive(archive, VERSION, target, self.notices)
        archive.write_bytes(original)
        def duplicate_key(entries):
            return [(entry, content.replace(b'{', b'{"product":"ignored",', 1)
                     if entry.filename == desktop.MANIFEST else content)
                    for entry, content in entries]
        self.rewrite_zip(archive, duplicate_key)
        with self.assertRaisesRegex(ValueError, "Duplicate desktop manifest key"):
            desktop.verify_archive(archive, VERSION, target, self.notices)

    def test_rehashed_macos_metadata_cannot_activate_unqualified_associations(self):
        target = "aarch64-apple-darwin"
        archive = self.package(target)
        def forged(entries):
            info_path = "TableCraft.app/Contents/Info.plist"
            changed = None
            for index, (entry, content) in enumerate(entries):
                if entry.filename == info_path:
                    metadata = plistlib.loads(content)
                    metadata["CFBundleDocumentTypes"] = [{"CFBundleTypeExtensions": ["mdb"]}]
                    changed = plistlib.dumps(metadata)
                    entries[index] = (entry, changed)
            for index, (entry, content) in enumerate(entries):
                if entry.filename == "TableCraft.app/Contents/Resources/RELEASE.json":
                    metadata = json.loads(content)
                    metadata["files"][info_path] = hashlib.sha256(changed).hexdigest()
                    entries[index] = (entry, json.dumps(metadata).encode())
            return entries
        self.rewrite_zip(archive, forged)
        with self.assertRaisesRegex(ValueError, "bundle metadata"):
            desktop.verify_archive(archive, VERSION, target, self.notices, self.icon)

    def test_smoke_executes_only_extracted_help_and_version_with_literal_paths(self):
        target = "x86_64-pc-windows-msvc"
        archive = self.package(target)
        replies = [subprocess.CompletedProcess([], 0, f"tablecraft-desktop {VERSION}\n", ""),
                   subprocess.CompletedProcess([], 0, "Usage: tablecraft-desktop [DATABASE]\n", "")]
        with patch.object(desktop.subprocess, "run", side_effect=replies) as run:
            desktop.verify_archive(archive, VERSION, target, self.notices, run_smoke=True)
        self.assertEqual([call.args[0][1:] for call in run.call_args_list], [["--version"], ["--help"]])
        for call in run.call_args_list:
            self.assertIsInstance(call.args[0], list)
            self.assertEqual(Path(call.args[0][0]).name, "tablecraft-desktop.exe")
            self.assertTrue(Path(call.args[0][0]).is_absolute())
            self.assertNotIn("shell", call.kwargs)

    def test_complete_desktop_set_has_independent_notices_and_checksums(self):
        for target in contract.TARGETS:
            self.package(target)
        output = self.root / "dist"
        release.finalize(VERSION, output, self.notices, self.icon)
        release.finalize(VERSION, output, self.notices, self.icon, verify_only=True)

        names = {path.name for path in output.iterdir()}
        self.assertEqual(len(names), 14)
        self.assertIn("DesktopThirdPartyLicenses.txt", names)
        self.assertIn("SHA256SUMS-desktop.txt", names)
        self.assertNotIn("ThirdPartyLicenses.txt", names)
        self.assertNotIn("SHA256SUMS.txt", names)
        self.assertEqual((output / release.NOTICES).read_bytes(), self.notices.read_bytes())
        manifest = (output / release.CHECKSUMS).read_text().splitlines()
        self.assertEqual(len(manifest), 13)
        for entry in manifest:
            digest, name = entry.split("  ")
            self.assertEqual(digest, hashlib.sha256((output / name).read_bytes()).hexdigest())

    def test_missing_platform_or_bad_sidecar_cannot_create_assembly_outputs(self):
        targets = list(contract.TARGETS)
        for target in targets[:-1]:
            self.package(target)
        output = self.root / "dist"
        with self.assertRaisesRegex(ValueError, "release set differs"):
            release.finalize(VERSION, output, self.notices, self.icon)
        self.assertFalse((output / release.NOTICES).exists())
        self.assertFalse((output / release.CHECKSUMS).exists())

        self.package(targets[-1])
        archive = output / desktop.archive_name(VERSION, targets[-1])
        archive.with_name(archive.name + ".sha256").write_text("wrong checksum\n")
        with self.assertRaisesRegex(ValueError, "sidecar differs"):
            release.finalize(VERSION, output, self.notices, self.icon)
        self.assertFalse((output / release.NOTICES).exists())
        self.assertFalse((output / release.CHECKSUMS).exists())

    def test_assembled_set_rejects_changed_notices_manifest_and_extra_assets(self):
        for target in contract.TARGETS:
            self.package(target)
        output = self.root / "dist"
        release.finalize(VERSION, output, self.notices, self.icon)
        notice_path = output / release.NOTICES
        original_notices = notice_path.read_bytes()
        notice_path.write_text("substituted notices\n")
        with self.assertRaisesRegex(ValueError, "Standalone desktop notices differ"):
            release.finalize(VERSION, output, self.notices, self.icon, verify_only=True)
        notice_path.write_bytes(original_notices)

        sums_path = output / release.CHECKSUMS
        original_sums = sums_path.read_bytes()
        sums_path.write_bytes(original_sums[:-1])
        with self.assertRaisesRegex(ValueError, "complete checksum manifest differs"):
            release.finalize(VERSION, output, self.notices, self.icon, verify_only=True)
        sums_path.write_bytes(original_sums)

        (output / "unreviewed-installer.exe").write_bytes(b"unexpected asset")
        with self.assertRaisesRegex(ValueError, "release set differs"):
            release.finalize(VERSION, output, self.notices, self.icon, verify_only=True)

    def test_failed_assembly_write_removes_only_its_new_outputs(self):
        for target in contract.TARGETS:
            self.package(target)
        output = self.root / "dist"
        before = {path.name: path.read_bytes() for path in output.iterdir()}
        original_open = Path.open

        def fail_manifest(path, mode="r", *args, **kwargs):
            if path == output / release.CHECKSUMS and mode == "xb":
                raise OSError("injected assembly write failure")
            return original_open(path, mode, *args, **kwargs)

        with patch.object(Path, "open", fail_manifest):
            with self.assertRaisesRegex(OSError, "injected assembly write failure"):
                release.finalize(VERSION, output, self.notices, self.icon)
        self.assertEqual({path.name: path.read_bytes() for path in output.iterdir()}, before)


if __name__ == "__main__":
    unittest.main()
