"""Independent native header/archive fixtures for the portable CLI release gate."""
import io
from pathlib import Path
import plistlib
import shutil
import struct
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

PACKAGING = Path(__file__).resolve().parents[1] / "packaging"
sys.path.insert(0, str(PACKAGING))
import package_cli as package
import release_contract as contract

VERSION = "0.1.0"


def native_binary(target):
    data = bytearray(256)
    if "windows" in target:
        data[:2] = b"MZ"
        struct.pack_into("<I", data, 60, 128)
        data[128:132] = b"PE\0\0"
        struct.pack_into("<H", data, 132, 0xaa64 if target.startswith("aarch64") else 0x8664)
        struct.pack_into("<H", data, 150, 0x0002)
    elif "apple" in target:
        data[:4] = bytes.fromhex("cffaedfe")
        struct.pack_into("<I", data, 4, 0x0100000c if target.startswith("aarch64") else 0x01000007)
        struct.pack_into("<I", data, 12, 2)
    else:
        data[:8] = b"\x7fELF\x02\x01\x01\x00"
        struct.pack_into("<H", data, 16, 3)
        struct.pack_into("<H", data, 18, 183 if target.startswith("aarch64") else 62)
    return bytes(data)


class PackagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tablecraft-packaging-test-")
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)
        (self.root / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{VERSION}"\n')
        notes = self.root / "docs" / "releases"
        notes.mkdir(parents=True)
        (notes / f"v{VERSION}.md").write_text("Original test release notes.\n")
        (self.root / "LICENSE").write_text("Original fixture license text.\n")
        resources = self.root / "packaging"
        resources.mkdir()
        (resources / "ThirdPartyLicenses.txt").write_text("Original fixture attribution.\n")
        for platform, name in [
            ("windows", "register-file-associations.ps1"),
            ("linux", "file_associations.py"),
        ]:
            destination = resources / platform / name
            destination.parent.mkdir()
            shutil.copyfile(PACKAGING.parents[1] / "packaging" / platform / name, destination)
        self.addCleanup(patch.stopall)
        patch.object(contract, "ROOT", self.root).start()

    def build_archive(self, target):
        binary = self.root / (target + ".bin")
        binary.write_bytes(native_binary(target))
        app = None
        if "apple" in target:
            app = self.root / target / "TableCraft.app"
            for name in contract.handler_names(target):
                destination = app / Path(name).relative_to("TableCraft.app")
                destination.parent.mkdir(parents=True, exist_ok=True)
                payload = (plistlib.dumps(contract.macos_info(VERSION))
                           if name == contract.MACOS_INFO else native_binary(target))
                destination.write_bytes(payload)
        return package.package(VERSION, target, binary, self.root / "dist", app)

    def test_six_independent_headers_and_wrong_architecture_refusals(self):
        for target in contract.TARGETS:
            data = native_binary(target)
            contract.architecture(io.BytesIO(data), len(data), target)
            opposite = target.replace("aarch64", "x86_64") if target.startswith("aarch64") else target.replace("x86_64", "aarch64")
            with self.assertRaises(ValueError, msg=target):
                contract.architecture(io.BytesIO(data), len(data), opposite)
        with self.assertRaises(ValueError):
            contract.architecture(io.BytesIO(b"not an executable"), 17, "x86_64-apple-darwin")

    def test_header_geometry_and_library_forms_are_refused(self):
        target = "x86_64-pc-windows-msvc"
        pe = bytearray(native_binary(target))
        struct.pack_into("<I", pe, 60, 0xfffffff0)
        with self.assertRaises(ValueError):
            contract.architecture(io.BytesIO(pe), len(pe), target)
        pe = bytearray(native_binary(target))
        struct.pack_into("<H", pe, 150, 0x2002)
        with self.assertRaises(ValueError):
            contract.architecture(io.BytesIO(pe), len(pe), target)
        macho = bytearray(native_binary("aarch64-apple-darwin"))
        struct.pack_into("<I", macho, 12, 6)
        with self.assertRaises(ValueError):
            contract.architecture(io.BytesIO(macho), len(macho), "aarch64-apple-darwin")

    def test_workspace_tag_and_release_notes_are_mandatory(self):
        contract.validate_version(VERSION, "v" + VERSION)
        for version, tag in [("9.0.0", None), (VERSION, VERSION), ("../bad", None)]:
            with self.assertRaises(ValueError):
                contract.validate_version(version, tag)
        (self.root / "docs" / "releases" / f"v{VERSION}.md").write_text(" \n")
        with self.assertRaises(ValueError):
            contract.validate_version(VERSION)

    def test_every_target_archive_contains_exact_legal_payload_and_binary(self):
        for target in contract.TARGETS:
            archive = self.build_archive(target)
            package.verify_archive(archive, VERSION, target)
            self.assertEqual(archive.with_name(archive.name + ".sha256").read_bytes(), package.checksum(archive))
            if "windows" in target:
                with zipfile.ZipFile(archive) as source:
                    names = source.namelist()
            else:
                with tarfile.open(archive) as source:
                    names = source.getnames()
            self.assertEqual(set(names), {
                contract.executable_name(target), "LICENSE", "ThirdPartyLicenses.txt",
                "RELEASE.json", *contract.handler_names(target),
            })

    def test_macos_app_cannot_be_missing_or_use_a_different_cli(self):
        target = "aarch64-apple-darwin"
        binary = self.root / "native"
        binary.write_bytes(native_binary(target))
        with self.assertRaisesRegex(ValueError, "Finder"):
            package.package(VERSION, target, binary, self.root / "missing")
        archive = self.build_archive(target)
        app = self.root / target / "TableCraft.app"
        bundled = app / "Contents" / "Resources" / "tablecraft"
        bundled.write_bytes(native_binary(target) + b"different binary")
        with self.assertRaisesRegex(ValueError, "differs"):
            package.package(VERSION, target, binary, self.root / "different", app)
        package.verify_archive(archive, VERSION, target)

    def test_macos_handler_claims_only_requested_extensions_as_alternate_viewer(self):
        info = contract.macos_info(VERSION)
        self.assertNotIn("UTImportedTypeDeclarations", info)
        self.assertNotIn("UTExportedTypeDeclarations", info)
        types = info["CFBundleDocumentTypes"]
        self.assertEqual([item["CFBundleTypeExtensions"] for item in types], [["mdb"], ["accdb"]])
        for item in types:
            self.assertEqual(item["CFBundleTypeRole"], "Viewer")
            self.assertEqual(item["LSHandlerRank"], "Alternate")
            self.assertNotIn("LSItemContentTypes", item)

    def test_complete_release_has_six_archives_six_sidecars_shared_licenses_and_complete_manifest(self):
        for target in contract.TARGETS:
            self.build_archive(target)
        directory = self.root / "dist"
        package.finalize(VERSION, directory)
        self.assertEqual(len(list(directory.iterdir())), 14)
        self.assertEqual(len((directory / package.SUMS).read_text().splitlines()), 13)
        package.finalize(VERSION, directory, verify_only=True)
        self.assertEqual(
            (directory / "ThirdPartyLicenses.txt").read_bytes(),
            (self.root / "packaging" / "ThirdPartyLicenses.txt").read_bytes(),
        )
        (directory / "stale.zip").write_text("stale")
        with self.assertRaises(ValueError):
            package.finalize(VERSION, directory, verify_only=True)

    def test_incomplete_release_never_creates_complete_manifest(self):
        self.build_archive("aarch64-apple-darwin")
        with self.assertRaises(ValueError):
            package.finalize(VERSION, self.root / "dist")
        self.assertFalse((self.root / "dist" / package.SUMS).exists())

    def test_modified_sidecar_or_archive_is_refused(self):
        for target in contract.TARGETS:
            self.build_archive(target)
        directory = self.root / "dist"
        sidecar = next(directory.glob("*.sha256"))
        sidecar.write_text("0" * 64 + "  wrong\n")
        with self.assertRaises(ValueError):
            package.finalize(VERSION, directory)
        self.assertFalse((directory / package.SUMS).exists())

    def test_archive_duplicate_and_unexpected_paths_are_refused(self):
        target = "x86_64-pc-windows-msvc"
        archive = self.build_archive(target)
        with zipfile.ZipFile(archive, "a") as stream:
            stream.writestr("../outside", b"not extracted")
        with self.assertRaises(ValueError):
            package.verify_archive(archive, VERSION, target)
        self.assertFalse((self.root / "outside").exists())

    def test_missing_and_changed_legal_resources_are_refused(self):
        target = "aarch64-apple-darwin"
        archive = self.build_archive(target)
        (self.root / "LICENSE").write_text("changed license")
        with self.assertRaises(ValueError):
            package.verify_archive(archive, VERSION, target)
        (self.root / "packaging" / "ThirdPartyLicenses.txt").unlink()
        with self.assertRaises(ValueError):
            contract.legal_payload()

    def test_symlinked_notice_is_refused_when_platform_permits_creation(self):
        notice = self.root / "packaging" / "ThirdPartyLicenses.txt"
        notice.unlink()
        try:
            notice.symlink_to(self.root / "LICENSE")
        except OSError as error:
            self.skipTest(f"Host does not permit creating symlink fixtures: {error}")
        with self.assertRaises(ValueError):
            contract.legal_payload()

    def test_duplicate_zip_entries_and_tar_links_are_refused(self):
        target = "x86_64-pc-windows-msvc"
        archive = self.build_archive(target)
        with self.assertWarns(UserWarning):
            with zipfile.ZipFile(archive, "a") as stream:
                stream.writestr("LICENSE", b"duplicate")
        with self.assertRaisesRegex(ValueError, "duplicate"):
            package.verify_archive(archive, VERSION, target)
        target = "aarch64-apple-darwin"
        archive = self.build_archive(target)
        with tarfile.open(archive, "w:gz") as stream:
            entry = tarfile.TarInfo("LICENSE")
            entry.type = tarfile.SYMTYPE
            entry.linkname = "../outside"
            stream.addfile(entry)
        with self.assertRaisesRegex(ValueError, "regular"):
            package.verify_archive(archive, VERSION, target)

    def test_fresh_asset_paths_prevent_overwrite(self):
        target = "aarch64-unknown-linux-gnu"
        archive = self.build_archive(target)
        before = archive.read_bytes()
        with self.assertRaises(ValueError):
            self.build_archive(target)
        self.assertEqual(before, archive.read_bytes())

    def test_extracted_smoke_checks_version_and_help(self):
        target = "aarch64-apple-darwin"
        archive = self.build_archive(target)
        responses = [
            type("Result", (), {"returncode": 0, "stderr": "", "stdout": f"tablecraft {VERSION}\n"})(),
            type("Result", (), {"returncode": 0, "stderr": "", "stdout": "Usage: tablecraft query\n"})(),
        ]
        with patch.object(package.subprocess, "run", side_effect=responses) as run:
            package.verify_archive(archive, VERSION, target, run_smoke=True)
        self.assertEqual([call.args[0][1] for call in run.call_args_list], ["--version", "--help"])
        self.assertTrue(all(call.kwargs["timeout"] == 30 for call in run.call_args_list))
        responses[0].stdout = "tablecraft wrong\n"
        with patch.object(package.subprocess, "run", return_value=responses[0]):
            with self.assertRaises(ValueError):
                package.verify_archive(archive, VERSION, target, run_smoke=True)


if __name__ == "__main__":
    unittest.main()
