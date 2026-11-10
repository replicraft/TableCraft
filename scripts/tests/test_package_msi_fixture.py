"""Fixture-isolation/version/publication checks; no claim of native MSI creation."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import msi_update_fixture as fixture
import package_windows as windows
import release_contract as contract
from test_package_cli import native_binary
from test_package_windows import icon_bytes


class MsiFixtureTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tablecraft-fixture-policy-")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.repository = self.base / "repository"
        self.repository.mkdir()
        (self.repository / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.0"\n')
        (self.repository / "LICENSE").write_text("First-party fixture license\n")
        notes = self.repository / "docs/releases"
        notes.mkdir(parents=True)
        (notes / "v0.1.0.md").write_text("Actual workspace version remains 0.1.0\n")
        self.addCleanup(patch.stopall)
        patch.object(contract, "ROOT", self.repository).start()
        self.binary = self.base / "tablecraft-desktop.exe"
        self.binary.write_bytes(native_binary("x86_64-pc-windows-msvc"))
        self.notices = self.base / "notices.txt"
        self.notices.write_text("First-party test notice\n")
        self.icon = self.base / "icon.ico"
        self.icon.write_bytes(icon_bytes())

    def build(self, output):
        return fixture.build("0.1.1", "x86_64-pc-windows-msvc", self.binary, self.notices, self.icon, output, "unexecuted-wix")

    def test_next_fixture_does_not_relax_the_production_workspace_contract(self):
        with self.assertRaisesRegex(ValueError, "matching workspace"):
            contract.validate_version("0.1.1")
        self.assertEqual(fixture.next_patch("0.1.0"), "0.1.1")
        with tempfile.TemporaryDirectory(dir=self.base) as temporary:
            with fixture.fixture_workspace(Path(temporary), "0.1.1", "0.1.0") as root:
                self.assertNotEqual(root, self.repository)
                contract.validate_version("0.1.1")
                self.assertEqual((root / "LICENSE").read_bytes(), (self.repository / "LICENSE").read_bytes())
                self.assertIn("TEST ONLY", (root / "docs/releases/v0.1.1.md").read_text())
        self.assertEqual(contract.ROOT, self.repository)
        with self.assertRaisesRegex(ValueError, "matching workspace"):
            contract.validate_version("0.1.1")

    def test_fixture_metadata_scope_restores_on_build_failure(self):
        output = self.base / "failed-fixture"
        with patch.object(windows, "package", side_effect=RuntimeError("independent simulated tool failure")):
            with self.assertRaisesRegex(RuntimeError, "simulated tool failure"):
                self.build(output)
        self.assertEqual(contract.ROOT, self.repository)
        self.assertFalse(output.exists())

    def test_only_test_named_receipt_and_carrier_are_published_outside_the_repository(self):
        def build(version, target, binary, notices, icon, output, tool):
            windows.preflight(version, target, binary, notices, icon)
            self.assertEqual(contract.workspace_version(), "0.1.1")
            output.mkdir()
            carrier = output / "unqualified-fixture-carrier.msi"
            carrier.write_bytes(b"First-party simulated carrier, not a native MSI")
            return carrier

        output = self.base / "fixture"
        with patch.object(windows, "package", side_effect=build):
            produced = self.build(output)
        self.assertEqual(produced.name, "TEST-ONLY-tablecraft-msi-update.msi")
        self.assertEqual({path.name for path in output.iterdir()}, {"TEST-ONLY-tablecraft-msi-update.msi", "TEST-ONLY.json"})
        receipt = json.loads((output / "TEST-ONLY.json").read_text())
        self.assertTrue(receipt["fixture_only"])
        self.assertFalse(receipt["release_asset"])
        self.assertEqual(receipt["binary_workspace_version"], "0.1.0")
        self.assertEqual(receipt["declared_fixture_version"], "0.1.1")
        self.assertEqual(contract.ROOT, self.repository)

    def test_repository_or_existing_outputs_refuse_before_any_tool(self):
        with patch.object(windows, "package") as build:
            with self.assertRaisesRegex(ValueError, "outside"):
                self.build(self.repository / "dist/fixture")
            existing = self.base / "existing"
            existing.mkdir()
            (existing / "owned.accdb").write_bytes(b"preserve")
            with self.assertRaises(FileExistsError):
                self.build(existing)
            build.assert_not_called()
        self.assertEqual((existing / "owned.accdb").read_bytes(), b"preserve")

    def test_other_fixture_versions_refuse_without_changing_workspace_metadata(self):
        with patch.object(windows, "package") as build:
            with self.assertRaisesRegex(ValueError, "next stable"):
                fixture.build("0.2.0", "x86_64-pc-windows-msvc", self.binary, self.notices, self.icon, self.base / "other")
            build.assert_not_called()
        self.assertEqual(contract.workspace_version(), "0.1.0")


if __name__ == "__main__":
    unittest.main()
