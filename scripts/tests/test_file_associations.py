"""Pure future-host payload checks; never installs or launches a handler."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "tablecraft_future_associations", ROOT / "scripts/packaging/file_associations.py",
)
renderer = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = renderer
SPEC.loader.exec_module(renderer)


class FileAssociationsTests(unittest.TestCase):
    def test_windows_uses_the_explicit_gui_executable_and_literal_file_argument(self):
        records = renderer.windows_records(r"C:\Program Files\TableCraft\Desktop.exe")
        commands = [item for item in records if item.key.endswith(r"\shell\open\command")]
        self.assertEqual(len(commands), 2)
        self.assertEqual(
            [(item.kind, item.value) for item in commands],
            [("REG_SZ", r'"C:\Program Files\TableCraft\Desktop.exe" -- "%1"')] * 2,
        )
        self.assertEqual(len(records), 11)
        self.assertFalse(any("--wait" in str(item.value) for item in records))

    def test_windows_records_offer_only_the_two_extensions_without_defaults(self):
        records = renderer.windows_records(r"D:\Apps\GUI.exe")
        self.assertTrue(all(item.key.startswith("HKEY_CURRENT_USER\\Software\\") for item in records))
        offers = [item for item in records if "\\Classes\\." in item.key]
        self.assertEqual(
            [(item.key, item.name, item.kind, item.value) for item in offers],
            [
                (r"HKEY_CURRENT_USER\Software\Classes\.mdb\OpenWithProgids",
                 "TableCraft.Database.mdb", "REG_NONE", b""),
                (r"HKEY_CURRENT_USER\Software\Classes\.accdb\OpenWithProgids",
                 "TableCraft.Database.accdb", "REG_NONE", b""),
            ],
        )
        self.assertFalse(any("UserChoice" in item.key for item in records))
        self.assertFalse(any(item.key.endswith(r"Classes\.mdb") for item in records))
        self.assertFalse(any(item.key.endswith(r"Classes\.accdb") for item in records))
        self.assertEqual(records[-1].value, r"Software\TableCraft\Capabilities")

    def test_windows_preserves_unicode_and_refuses_ambiguous_or_non_native_paths(self):
        path = r"C:\用户\a & b '$();\Desktop.exe"
        commands = [item.value for item in renderer.windows_records(path)
                    if item.key.endswith(r"\shell\open\command")]
        self.assertEqual(commands, ['"' + path + '" -- "%1"'] * 2)
        for invalid in (
            "Desktop.exe", r"C:Desktop.exe", r"\Apps\Desktop.exe",
            r"C:\Apps\Desktop.cmd", r"C:\Apps\%1\Desktop.exe",
            'C:\\Apps\\bad"name.exe', "C:\\Apps\\bad\nname.exe", "C:\\Apps\\bad\0name.exe",
        ):
            with self.subTest(path=invalid), self.assertRaises(ValueError):
                renderer.windows_records(invalid)

    def test_linux_payloads_use_a_graphical_host_and_the_existing_mime_identity(self):
        payloads = renderer.linux_payloads("/opt/TableCraft/Desktop")
        self.assertEqual(set(payloads), {
            "applications/org.replicraft.tablecraft.desktop",
            "mime/packages/org.replicraft.tablecraft.xml",
        })
        self.assertEqual(
            payloads["applications/org.replicraft.tablecraft.desktop"],
            b'[Desktop Entry]\nType=Application\nName=TableCraft\n'
            b'Exec="/opt/TableCraft/Desktop" -- %f\nTerminal=false\n'
            b'MimeType=application/vnd.ms-access;\nCategories=Office;Database;\n',
        )
        mime = ET.fromstring(payloads["mime/packages/org.replicraft.tablecraft.xml"])
        namespace = {"m": "http://www.freedesktop.org/standards/shared-mime-info"}
        self.assertEqual(mime.find("m:mime-type", namespace).attrib,
                         {"type": "application/vnd.ms-access"})
        self.assertEqual(
            [item.attrib for item in mime.findall(".//m:glob", namespace)],
            [{"pattern": "*.mdb"}, {"pattern": "*.accdb"}],
        )

    def test_linux_exec_escaping_has_distinct_field_code_and_quoting_layers(self):
        path = '/opt/用户/a "b" $`%\\Desktop'
        executable = renderer._desktop_executable(path)
        expected = '"/opt/用户/a \\\\"b\\\\" \\\\$\\\\`%%\\\\\\\\Desktop"'
        self.assertEqual(executable, expected)
        payload = renderer.linux_payloads(path)["applications/org.replicraft.tablecraft.desktop"]
        self.assertIn(("Exec=" + expected + " -- %f\n").encode(), payload)
        self.assertNotIn(b"sh -c", payload)

    def test_linux_refuses_relative_directory_and_control_paths(self):
        for invalid in ("Desktop", "/", "/opt/", "/opt/a=b", "/opt/a\nb", "/opt/a\0b", "/a/../Desktop"):
            with self.subTest(path=invalid), self.assertRaises(ValueError):
                renderer.linux_payloads(invalid)

    def test_macos_fragment_claims_exact_extensions_without_competing_utis(self):
        self.assertEqual(renderer.macos_document_types(), {
            "CFBundleDocumentTypes": [
                {
                    "CFBundleTypeName": "Microsoft Access MDB database",
                    "CFBundleTypeExtensions": ["mdb"],
                    "CFBundleTypeRole": "Viewer",
                    "LSHandlerRank": "Alternate",
                },
                {
                    "CFBundleTypeName": "Microsoft Access ACCDB database",
                    "CFBundleTypeExtensions": ["accdb"],
                    "CFBundleTypeRole": "Viewer",
                    "LSHandlerRank": "Alternate",
                },
            ],
        })

    def test_rendering_never_reads_files_spawns_processes_or_changes_the_host(self):
        with patch.object(Path, "open", side_effect=AssertionError("filesystem access")), \
                patch.object(os, "stat", side_effect=AssertionError("filesystem access")), \
                patch.object(subprocess, "run", side_effect=AssertionError("process launch")):
            renderer.windows_records(r"C:\Explicit\FutureGUI.exe")
            renderer.linux_payloads("/explicit/FutureGUI")
            renderer.macos_document_types()
        self.assertFalse(hasattr(renderer, "install"))
        self.assertFalse(hasattr(renderer, "apply"))


if __name__ == "__main__":
    unittest.main()
