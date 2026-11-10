"""Custom DMG policy and ownership checks; native mount evidence is recorded separately."""
import importlib.util
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch
import zipfile

PACKAGING = Path(__file__).resolve().parents[1] / "packaging"
sys.path.insert(0, str(PACKAGING))
import package_desktop as desktop
import package_macos as macos
import installer_contract as installers
import release_contract as contract
from test_package_cli import VERSION, native_binary
from test_package_desktop import outline_icon


class MacosPackagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tablecraft-dmg-contract-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{VERSION}"\n')
        (self.root / "docs/releases").mkdir(parents=True)
        (self.root / "docs/releases" / f"v{VERSION}.md").write_text("Independent DMG contract.\n")
        (self.root / "LICENSE").write_text("Independent project license.\n")
        self.notices = self.root / "notices.txt"
        self.notices.write_text("Independent notice input.\n")
        self.icon = self.root / "application.icns"
        self.icon.write_bytes(outline_icon())
        self.addCleanup(patch.stopall)
        patch.object(contract, "ROOT", self.root).start()
        self.target = "aarch64-apple-darwin"
        self.archive = self.source(self.target)
        self.background = self.root / "background.png"
        self.background.write_bytes(b"independent first-party artwork bytes")

    def source(self, target):
        binary = self.root / (target + ".bin")
        binary.write_bytes(native_binary(target))
        return desktop.package(VERSION, target, binary, self.notices, self.root / "portable", self.icon)

    def volume(self, name="volume"):
        volume = self.root / name
        volume.mkdir()
        with zipfile.ZipFile(self.archive) as stream:
            for entry in stream.infolist():
                destination = volume / entry.filename
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(stream.read(entry))
                destination.chmod(entry.external_attr >> 16 & 0o777)
        (volume / "Applications").symlink_to("/Applications")
        (volume / ".background.png").write_bytes(self.background.read_bytes())
        (volume / ".VolumeIcon.icns").write_bytes(self.icon.read_bytes())
        (volume / ".DS_Store").write_bytes(b"independent metadata carrier")
        return volume

    def test_both_architectures_publish_fresh_images_from_checked_application_bytes(self):
        for target in (self.target, "x86_64-apple-darwin"):
            archive = self.archive if target == self.target else self.source(target)
            output = self.root / target

            def build(app, image, icon, background):
                with zipfile.ZipFile(archive) as source:
                    for entry in source.infolist():
                        self.assertEqual((app.parent / entry.filename).read_bytes(), source.read(entry))
                self.assertEqual(icon, self.icon)
                self.assertEqual(background.read_bytes(), b"generated background")
                image.write_bytes(b"independent tool-produced disk image")

            def draw(path):
                path.write_bytes(b"generated background")

            with (
                patch.object(macos, "build_disk_image", side_effect=build),
                patch.object(macos, "artwork", side_effect=draw),
                patch.object(macos, "verify") as verify,
            ):
                image = macos.package(archive, VERSION, target, self.notices, self.icon, output)
            self.assertEqual(image.name, installers.name(VERSION, target, "dmg"))
            self.assertEqual(image.with_name(image.name + ".sha256").read_bytes(), installers.checksum(image))
            verify.assert_called_once()
            with self.assertRaisesRegex(ValueError, "fresh paths"):
                macos.package(archive, VERSION, target, self.notices, self.icon, output)

    def test_invalid_source_and_failed_construction_never_publish_an_image(self):
        output = self.root / "refused"
        with zipfile.ZipFile(self.archive, "a") as stream:
            stream.writestr("unlisted-resource", b"not part of the package")
        with patch.object(macos, "build_disk_image") as build:
            with self.assertRaises(ValueError):
                macos.package(self.archive, VERSION, self.target, self.notices, self.icon, output)
            build.assert_not_called()
        self.assertFalse(output.exists())

    def test_publish_race_does_not_replace_another_process_image(self):
        output = self.root / "race"
        destination = output / installers.name(VERSION, self.target, "dmg")

        def build(app, image, icon, background):
            image.write_bytes(b"our verified image")

        def verify(*arguments):
            destination.write_bytes(b"another process image")

        with (
            patch.object(macos, "build_disk_image", side_effect=build),
            patch.object(macos, "artwork", side_effect=lambda path: path.write_bytes(b"background")),
            patch.object(macos, "verify", side_effect=verify),
        ):
            with self.assertRaises(FileExistsError):
                macos.package(self.archive, VERSION, self.target, self.notices, self.icon, output)
        self.assertEqual(destination.read_bytes(), b"another process image")
        self.assertFalse(destination.with_name(destination.name + ".sha256").exists())
        self.assertEqual({path.name for path in output.iterdir()}, {destination.name})

    def test_failed_verification_cleans_owned_staging(self):
        output = self.root / "failed"
        with (
            patch.object(macos, "build_disk_image", side_effect=lambda app, image, icon, background: image.write_bytes(b"invalid image")),
            patch.object(macos, "artwork", side_effect=lambda path: path.write_bytes(b"background")),
            patch.object(macos, "verify", side_effect=ValueError("independent verification failure")),
        ):
            with self.assertRaisesRegex(ValueError, "independent verification"):
                macos.package(self.archive, VERSION, self.target, self.notices, self.icon, output)
        self.assertEqual(list(output.iterdir()), [])
        self.assertTrue(self.archive.is_file())

    def test_dmgbuild_attach_is_nonopening_and_wrapper_is_restored_on_failure(self):
        calls = []
        core = types.SimpleNamespace(hdiutil=lambda *args, **kwargs: calls.append((args, kwargs)))
        original = core.hdiutil
        module = types.ModuleType("dmgbuild")
        module.core = core

        def build(*arguments, **options):
            core.hdiutil("attach", "-nobrowse", "owned-image", plist=True)
            self.assertEqual(options["settings"]["hide_extensions"], [])
            raise ValueError("independent builder failure")

        module.build_dmg = build
        with (
            patch.dict(sys.modules, {"dmgbuild": module}),
            patch.object(macos.sys, "platform", "darwin"),
        ):
            with self.assertRaisesRegex(ValueError, "independent builder"):
                macos.build_disk_image(self.root / "TableCraft.app", self.root / "image.dmg", self.icon, self.background)
        self.assertIs(core.hdiutil, original)
        self.assertEqual(calls, [(("attach", "-noautoopen", "-nobrowse", "owned-image"), {"plist": True})])

    def test_readonly_mount_detaches_its_owned_path_when_verification_raises(self):
        calls = []

        def command(*arguments):
            calls.append(arguments)
            if arguments[0] == "attach":
                mount = arguments[arguments.index("-mountpoint") + 1]
                return subprocess.CompletedProcess([], 0, plistlib.dumps({
                    "system-entities": [{"dev-entry": "/dev/owned-fixture", "mount-point": str(mount)}],
                }))
            return subprocess.CompletedProcess([], 0, b"")

        with patch.object(macos, "hdiutil", side_effect=command):
            with self.assertRaisesRegex(ValueError, "injected payload"):
                with macos.mounted(self.root / "input.dmg", self.root):
                    raise ValueError("injected payload failure")
        self.assertTrue(all(flag in calls[0] for flag in ("-readonly", "-nobrowse", "-noautoopen")))
        self.assertEqual(calls[-1], ("detach", "-quiet", self.root / "volume"))

    def test_wrong_native_image_format_refuses_before_mounting(self):
        image = self.root / installers.name(VERSION, self.target, "dmg")
        image.write_bytes(b"koly" + bytes(508))
        for kind, encrypted in (("UDRW", False), ("UDZO", True)):
            information = {
                "Format": kind,
                "Properties": {"Compressed": True, "Checksummed": True, "Encrypted": encrypted},
            }
            reply = subprocess.CompletedProcess([], 0, plistlib.dumps(information))
            with (
                self.subTest(kind=kind, encrypted=encrypted),
                patch.object(macos.sys, "platform", "darwin"),
                patch.object(macos, "hdiutil", return_value=reply),
                patch.object(macos, "mounted") as mounted,
            ):
                with self.assertRaisesRegex(ValueError, "release format"):
                    macos.verify(image, self.archive, VERSION, self.target, self.notices, self.icon)
                mounted.assert_not_called()

    @unittest.skipIf(sys.platform == "win32", "Fixture symlinks do not require Windows privileges")
    def test_mounted_payload_is_exact_and_refuses_changes_links_and_extra_directories(self):
        with patch.object(macos, "validate_store"):
            volume = self.volume()
            macos.validate_mounted(volume, self.archive, self.icon, self.background)
        cases = ("changed-app", "wrong-applications", "extra-file", "extra-directory", "app-symlink")
        for case in cases:
            volume = self.volume(case)
            if case == "changed-app":
                (volume / "TableCraft.app/Contents/Resources/LICENSE").write_bytes(b"changed license")
            elif case == "wrong-applications":
                (volume / "Applications").unlink()
                (volume / "Applications").symlink_to("/not-applications")
            elif case == "extra-file":
                (volume / "extra.sh").write_bytes(b"unowned")
            elif case == "extra-directory":
                (volume / "TableCraft.app/Contents/unlisted").mkdir()
            else:
                shutil.rmtree(volume / "TableCraft.app")
                (volume / "TableCraft.app").symlink_to(self.root)
            with self.subTest(case=case), patch.object(macos, "validate_store"):
                with self.assertRaises(ValueError):
                    macos.validate_mounted(volume, self.archive, self.icon, self.background)

    @unittest.skipUnless(importlib.util.find_spec("ds_store"), "DS_Store tool dependency is not installed")
    def test_actual_ds_store_layout_round_trip_and_icon_position_tampering(self):
        from ds_store import DSStore
        path = self.root / "layout.DS_Store"
        with DSStore.open(str(path), "w+") as store:
            store["TableCraft.app"]["Iloc"] = (180, 185)
            store["Applications"]["Iloc"] = (540, 185)
            store["."]["icvp"] = {"iconSize": 112.0, "backgroundType": 2}
            store["."]["bwsp"] = {
                "WindowBounds": "{{160, 180}, {720, 400}}",
                "ShowStatusBar": False, "ShowToolbar": False, "ShowPathbar": False,
                "ShowSidebar": False, "ShowTabView": False,
            }
        macos.validate_store(path)
        with DSStore.open(str(path), "r+") as store:
            store["Applications"]["Iloc"] = (2, 2)
        with self.assertRaisesRegex(ValueError, "icon placement"):
            macos.validate_store(path)

    @unittest.skipUnless(importlib.util.find_spec("PIL"), "Pillow tool dependency is not installed")
    def test_original_sized_background_is_deterministic_and_uses_the_bundled_fonts(self):
        from PIL import Image
        first, second = self.root / "first.png", self.root / "second.png"
        macos.artwork(first)
        macos.artwork(second)
        self.assertEqual(first.read_bytes(), second.read_bytes())
        with Image.open(first) as image:
            self.assertEqual(image.size, (720, 400))
            self.assertEqual(image.mode, "RGB")


if __name__ == "__main__":
    unittest.main()
