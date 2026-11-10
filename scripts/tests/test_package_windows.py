"""MSI input/ownership and fault tests; these do not claim native Windows builds."""
import copy
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

PACKAGING = Path(__file__).resolve().parents[1] / "packaging"
sys.path.insert(0, str(PACKAGING))
import file_associations
import installer_contract
import package_windows as windows
import release_contract as contract
from test_package_cli import native_binary
from test_package_desktop import outline_icon

VERSION = "0.1.0"
TARGET = "x86_64-pc-windows-msvc"


def icon_bytes():
    png = outline_icon()[16:]
    return struct.pack("<HHH", 0, 1, 1) + struct.pack("<BBBBHHII", 16, 16, 0, 0, 1, 32, len(png), 22) + png


class WindowsInstallerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tablecraft-windows-packaging-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.0"\n')
        (self.root / "docs/releases").mkdir(parents=True)
        (self.root / "docs/releases/v0.1.0.md").write_text("Original test release notes.\n")
        (self.root / "LICENSE").write_text("Original project license.\n")
        self.binary = self.root / "tablecraft-desktop.exe"
        self.binary.write_bytes(native_binary(TARGET))
        self.notices = self.root / "desktop-notices.txt"
        self.notices.write_text("Original dependency notice.\n")
        self.icon = self.root / "TableCraft.ico"
        self.icon.write_bytes(icon_bytes())
        self.output = self.root / "output"
        self.addCleanup(patch.stopall)
        patch.object(contract, "ROOT", self.root).start()

    def stage(self, target=TARGET):
        stage = self.root / "payload"
        windows.stage_payload(VERSION, target, self.binary, self.notices, stage)
        return stage

    def report(self, payload):
        file_rows = [
            {"id": "TableCraftDesktop", "component": "DesktopApplication", "name": "tablecraft-desktop.exe"},
            {"id": "ProjectLicenseFile", "component": "ProjectLicense", "name": "LICENSE"},
            {"id": "ThirdPartyNoticesFile", "component": "ThirdPartyNotices", "name": "ThirdPartyLicenses.txt"},
            {"id": "ReleaseManifestFile", "component": "ReleaseManifest", "name": "RELEASE.json"},
        ]
        for row in file_rows:
            row["size"] = str((payload / row["name"]).stat().st_size)
        components = [
            {"id": row["component"], "directory": "INSTALLFOLDER", "attributes": "256", "keypath": row["id"]}
            for row in file_rows
        ]
        components.append({"id": "ApplicationRegistration", "directory": "INSTALLFOLDER", "attributes": "260", "keypath": "RegistrationValue00"})
        return {
            "template": "x64;1033",
            "customActions": [],
            "properties": [{"name": key, "value": value} for key, value in {
                "ProductName": "TableCraft", "Manufacturer": "Replicraft", "ProductVersion": VERSION,
                "ALLUSERS": "1", "ARPPRODUCTICON": "TableCraftIcon", "UpgradeCode": windows.UPGRADE_CODE,
            }.items()],
            "files": file_rows,
            "components": components,
            "directories": [{"id": "INSTALLFOLDER", "parent": "ProgramFiles64Folder", "name": "TableCraft"}],
            "shortcuts": [{"id": "TableCraftStartMenu", "directory": "ProgramMenuFolder", "name": "TableCraft",
                           "component": "DesktopApplication", "target": "DesktopFeature", "icon": "TableCraftIcon"}],
            "registry": windows.registry_values(),
            "upgrades": [{"code": windows.UPGRADE_CODE, "action": action, "minimum": "", "maximum": "", "attributes": "0"} for action in ("WIX_UPGRADE_DETECTED", "WIX_DOWNGRADE_DETECTED")],
            "icons": [{"name": "TableCraftIcon"}],
        }

    def test_actual_pe_headers_and_real_icon_pixels_are_required_for_both_architectures(self):
        windows.preflight(VERSION, TARGET, self.binary, self.notices, self.icon)
        arm = "aarch64-pc-windows-msvc"
        with self.assertRaises(ValueError):
            windows.preflight(VERSION, arm, self.binary, self.notices, self.icon)
        self.binary.write_bytes(native_binary(arm))
        windows.preflight(VERSION, arm, self.binary, self.notices, self.icon)
        self.icon.write_bytes(b"not an icon")
        with self.assertRaises(ValueError):
            windows.preflight(VERSION, arm, self.binary, self.notices, self.icon)
        self.assertFalse(self.output.exists())

    def test_staged_snapshot_has_exact_owned_files_and_manifest_hashes(self):
        payload = self.stage()
        self.assertEqual({path.name for path in payload.iterdir()}, {"tablecraft-desktop.exe", "LICENSE", "ThirdPartyLicenses.txt", "RELEASE.json"})
        manifest = json.loads((payload / "RELEASE.json").read_text())
        self.assertEqual(manifest["product"], "tablecraft-desktop")
        self.assertEqual(manifest["version"], VERSION)
        self.assertEqual(manifest["target"], TARGET)
        self.assertEqual(manifest["installation"], "windows-msi")
        for name, digest in manifest["files"].items():
            self.assertEqual(digest, contract.digest(payload / name))
        before = (payload / "tablecraft-desktop.exe").read_bytes()
        self.binary.write_bytes(b"changed after staging")
        self.assertEqual((payload / "tablecraft-desktop.exe").read_bytes(), before)

    def test_handler_registration_reuses_progids_and_quotes_literal_filename_without_default_takeover(self):
        values = windows.registry_values()
        self.assertEqual(len(values), len(file_associations.windows_records(windows.COMMAND_PLACEHOLDER)))
        for extension in ("mdb", "accdb"):
            progid = f"TableCraft.Database.{extension}"
            command = next(value for value in values if value["key"] == f"Software\\Classes\\{progid}\\shell\\open\\command")
            self.assertEqual(command["value"], '"[#TableCraftDesktop]" -- "%1"')
            self.assertEqual(command["root"], "2")
            open_with = next(value for value in values if value["key"] == f"Software\\Classes\\.{extension}\\OpenWithProgids")
            self.assertEqual(open_with["name"], progid)
            self.assertEqual(open_with["value"], "")
        self.assertFalse(any(value["key"] in ("Software\\Classes\\.mdb", "Software\\Classes\\.accdb") for value in values))
        self.assertFalse(any("UserChoice" in value["key"] for value in values))
        registration = self.root / "registration.wxi"
        windows.write_registry(registration)
        tree = ET.parse(registration)
        records = list(tree.getroot())
        self.assertEqual(sum(record.get("KeyPath") == "yes" for record in records), 1)
        self.assertTrue(all(record.get("Root") == "HKLM" for record in records))

    def test_template_has_owned_components_shortcut_and_transactional_upgrade_without_autolaunch(self):
        tree = ET.parse(windows.TEMPLATE)
        namespace = {"w": windows.WIX_NAMESPACE}
        package = tree.find("w:Package", namespace)
        self.assertEqual(package.get("Scope"), "perMachine")
        upgrade = package.find("w:MajorUpgrade", namespace)
        self.assertEqual(upgrade.get("Schedule"), "afterInstallInitialize")
        self.assertIsNone(upgrade.get("AllowSameVersionUpgrades"))
        self.assertEqual(len(package.findall(".//w:File", namespace)), 4)
        self.assertEqual(len(package.findall(".//w:Shortcut", namespace)), 1)
        self.assertFalse(package.findall(".//w:CustomAction", namespace))
        self.assertFalse(package.findall(".//w:RemoveFile", namespace))
        self.assertTrue(all(component.get("Bitness") == "always64" for component in package.findall(".//w:Component", namespace)))

    def test_wix_arguments_keep_paths_as_distinct_arguments(self):
        stage = self.root / "payload ' ; $(bad)"
        command = windows.build_command(VERSION, TARGET, stage, self.icon, self.root / "include.wxi", self.root / "out.msi", "wix")
        self.assertEqual(command[:4], ["wix", "build", "-arch", "x64"])
        self.assertIn(f"Payload={stage}", command)
        self.assertEqual(command[-1], str(windows.TEMPLATE))
        arm = windows.build_command(VERSION, "aarch64-pc-windows-msvc", stage, self.icon, self.root / "include.wxi", self.root / "out.msi", "wix")
        self.assertEqual(arm[3], "arm64")

    def test_icon_offsets_and_overlapping_images_refuse(self):
        self.icon.write_bytes(icon_bytes()[:20])
        with self.assertRaises(ValueError):
            windows.validate_icon(self.icon)
        png = icon_bytes()[22:]
        entry = struct.pack("<BBBBHHII", 16, 16, 0, 0, 1, 32, len(png), 38)
        self.icon.write_bytes(struct.pack("<HHH", 0, 1, 2) + entry + entry + png)
        with self.assertRaisesRegex(ValueError, "Overlapping"):
            windows.validate_icon(self.icon)

    def test_table_checks_refuse_architecture_payload_lifetime_and_registration_changes(self):
        payload = self.stage()
        report = self.report(payload)
        windows.check_tables(report, VERSION, TARGET, payload)
        changes = [
            lambda value: value.update(template="Arm64;1033"),
            lambda value: value["files"][0].update(name="other.exe"),
            lambda value: value["components"][0].update(attributes="272"),
            lambda value: value["components"][0].update(directory="OTHER"),
            lambda value: value["registry"][0].update(value="other handler"),
            lambda value: value["shortcuts"][0].update(target="OtherFeature"),
            lambda value: value.update(upgrades=[]),
            lambda value: value.update(icons=[]),
            lambda value: value.update(customActions=[{"action": "LaunchUnexpected"}]),
        ]
        for change in changes:
            modified = copy.deepcopy(report)
            change(modified)
            with self.assertRaises(ValueError):
                windows.check_tables(modified, VERSION, TARGET, payload)

    def test_com_method_return_values_cannot_be_interpreted_as_installer_rows(self):
        payload = self.stage()
        for table in ("files", "components", "properties", "directories", "registry"):
            report = self.report(payload)
            report[table].insert(0, None)
            with self.assertRaisesRegex(ValueError, f"{table} row 0"):
                windows.check_tables(report, VERSION, TARGET, payload)

    def test_raw_native_inspection_is_retained_before_table_validation(self):
        evidence = self.root / "inspection-evidence"
        raw = b'{"files":[null]}'
        result = subprocess.CompletedProcess([], 0, raw)
        with patch.dict(windows.os.environ, {"TABLECRAFT_MSI_EVIDENCE_DIRECTORY": str(evidence)}), patch.object(windows.subprocess, "run", return_value=result):
            report = windows.inspect_tables(self.root / "unexecuted.msi")
        self.assertEqual(report, {"files": [None]})
        self.assertEqual((evidence / "inspection.json").read_bytes(), raw)

    def test_windows_installer_short_and_long_name_encoding_preserves_receiver_identity(self):
        payload = self.stage()
        report = self.report(payload)
        report["shortcuts"][0]["name"] = "TABLEC~1|TableCraft"
        report["directories"][0]["name"] = "TABLEC~1|TableCraft"
        report["files"][0]["name"] = "TABLEC~1.EXE|tablecraft-desktop.exe"
        windows.check_tables(report, VERSION, TARGET, payload)

    def test_tool_failure_preserves_existing_outputs_and_removes_only_own_staging(self):
        self.output.mkdir()
        sentinel = self.output / "unrelated.accdb"
        sentinel.write_bytes(b"keep this database")
        with patch.object(windows.sys, "platform", "win32"), patch.object(windows.subprocess, "run", side_effect=subprocess.CalledProcessError(1, ["wix"])):
            with self.assertRaises(subprocess.CalledProcessError):
                windows.package(VERSION, TARGET, self.binary, self.notices, self.icon, self.output)
        self.assertEqual({path.name for path in self.output.iterdir()}, {"unrelated.accdb"})
        self.assertEqual(sentinel.read_bytes(), b"keep this database")

    def test_checksum_collision_does_not_remove_the_foreign_file_or_publish_partial_output(self):
        self.output.mkdir()
        installer = self.output / installer_contract.name(VERSION, TARGET, "msi")
        checksum = installer.with_name(installer.name + ".sha256")

        def build(command, **options):
            destination = Path(command[command.index("-o") + 1])
            destination.write_bytes(b"mocked tool output, not a qualified MSI")

        def collide(*args, **options):
            checksum.write_bytes(b"foreign concurrent output")

        with patch.object(windows.sys, "platform", "win32"), patch.object(windows.subprocess, "run", side_effect=build), patch.object(windows, "verify", side_effect=collide):
            with self.assertRaises(FileExistsError):
                windows.package(VERSION, TARGET, self.binary, self.notices, self.icon, self.output)
        self.assertFalse(installer.exists())
        self.assertEqual(checksum.read_bytes(), b"foreign concurrent output")


if __name__ == "__main__":
    unittest.main()
