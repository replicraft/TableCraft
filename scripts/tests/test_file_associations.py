"""Pure registration rendering/ownership tests; never changes host associations."""
import errno
import importlib.util
from pathlib import Path, PurePosixPath
import tempfile
import subprocess
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    'file_associations', ROOT / 'packaging/linux/file_associations.py',
)
linux = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(linux)


class FileAssociationsTests(unittest.TestCase):
    def test_desktop_command_and_mime_types(self):
        files = linux.payloads(PurePosixPath('/opt/TableCraft/tablecraft'))
        desktop = files['applications/org.tablecraft.Inspect.desktop'].decode()
        self.assertIn('Exec="/opt/TableCraft/tablecraft" open --wait -- %f\n', desktop)
        self.assertIn('Terminal=true\n', desktop)
        self.assertIn('MimeType=application/vnd.ms-access;', desktop)
        self.assertNotIn('application/x-tablecraft-access', desktop)
        self.assertNotIn('sh -c', desktop)
        self.assertNotIn('Default Applications', desktop)
        xml = ET.fromstring(files['mime/packages/org.tablecraft.Inspect.xml'])
        namespace = {'m': 'http://www.freedesktop.org/standards/shared-mime-info'}
        self.assertEqual(xml.find('m:mime-type', namespace).attrib['type'], 'application/vnd.ms-access')
        self.assertEqual([item.attrib['pattern'] for item in xml.findall('.//m:glob', namespace)],
                         ['*.mdb', '*.accdb'])

    def test_exec_escaping_preserves_unicode_and_special_characters(self):
        path = Path('/home/用户/a b/"`$%\\tablecraft')
        encoded = linux.executable_argument(path)
        # Undo desktop string escaping, then Exec quoting, without invoking a shell.
        value = encoded[1:-1].replace('\\\\', '\\').replace('%%', '%')
        decoded = []
        index = 0
        while index < len(value):
            if value[index] == '\\':
                index += 1
            decoded.append(value[index])
            index += 1
        self.assertEqual(''.join(decoded), str(path))
        for invalid in ('/a=b/tablecraft', '/a\n/tablecraft'):
            with self.assertRaises(ValueError):
                linux.executable_argument(Path(invalid))

    def test_install_idempotence_uninstall_and_default_preservation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            defaults = root / 'applications/mimeapps.list'
            defaults.parent.mkdir()
            defaults.write_bytes(b'[Default Applications]\napplication/vnd.ms-access=other.desktop;\n')
            before = defaults.read_bytes()
            with patch.object(linux.shutil, 'which', return_value='/mock/tool'), \
                    patch.object(linux.subprocess, 'run') as run:
                linux.apply('install', Path('/opt/tablecraft'), root)
                linux.apply('install', Path('/opt/tablecraft'), root)
                self.assertEqual(run.call_count, 4)
                linux.apply('uninstall', Path('/opt/tablecraft'), root)
                self.assertEqual(run.call_count, 6)
            self.assertEqual(defaults.read_bytes(), before)
            self.assertFalse((root / linux.RECEIPT).exists())
            self.assertFalse((root / 'applications' / linux.DESKTOP_ID).exists())

    def test_unowned_changed_and_other_installation_are_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / 'applications' / linux.DESKTOP_ID
            target.parent.mkdir()
            target.write_bytes(b'other')
            with self.assertRaises(ValueError):
                linux.apply('install', Path('/opt/tablecraft'), root)
            target.unlink()
            with patch.object(linux.shutil, 'which', return_value='/mock/tool'), \
                    patch.object(linux.subprocess, 'run'):
                linux.apply('install', Path('/opt/tablecraft'), root)
                with self.assertRaises(ValueError):
                    linux.apply('uninstall', Path('/else/tablecraft'), root)
                target.write_bytes(b'modified')
                with self.assertRaises(ValueError):
                    linux.apply('uninstall', Path('/opt/tablecraft'), root)
                self.assertEqual(target.read_bytes(), b'modified')
                self.assertTrue((root / linux.RECEIPT).exists())

    def test_symlinks_refuse_before_writes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            try:
                (root / 'applications').symlink_to(root / 'elsewhere')
            except (OSError, NotImplementedError) as error:
                unavailable = (errno.EPERM, errno.EACCES, errno.ENOSYS, errno.ENOTSUP)
                if isinstance(error, OSError):
                    if error.errno not in unavailable and getattr(error, 'winerror', None) != 1314:
                        raise
                self.skipTest(f'Symlink fixture is unavailable: {error}')
            with self.assertRaises(ValueError):
                linux.apply('install', Path('/opt/tablecraft'), root)
            (root / 'applications').unlink()

    def test_missing_tools_refuse_before_writes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(linux.shutil, 'which', return_value=None):
                with self.assertRaises(ValueError):
                    linux.apply('install', Path('/opt/tablecraft'), root)
            self.assertEqual(list(root.iterdir()), [])

    def test_refresh_failure_retains_recoverable_ownership(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(linux.shutil, 'which', return_value='/mock/tool'), \
                    patch.object(linux.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, 'cache')):
                with self.assertRaises(subprocess.CalledProcessError):
                    linux.apply('install', Path('/opt/tablecraft'), root)
            self.assertTrue((root / linux.RECEIPT).exists())
            with patch.object(linux.shutil, 'which', return_value='/mock/tool'), \
                    patch.object(linux.subprocess, 'run'):
                linux.apply('uninstall', Path('/opt/tablecraft'), root)
            self.assertFalse((root / linux.RECEIPT).exists())

    def test_windows_registration_is_per_user_and_never_sets_defaults(self):
        text = (ROOT / 'packaging/windows/register-file-associations.ps1').read_text()
        self.assertIn('[Microsoft.Win32.Registry]::CurrentUser', text)
        self.assertIn('open --wait -- "%1"', text)
        self.assertIn('OpenWithProgids', text)
        self.assertIn('Software\\RegisteredApplications', text)
        self.assertIn('DoNotExpandEnvironmentNames', text)
        self.assertNotIn('UserChoice', text)
        self.assertNotIn('LocalMachine', text)
        self.assertNotIn('DeleteSubKeyTree', text)
        self.assertNotIn('cmd.exe', text)
        self.assertIn('$Owner -cne $Executable', text)
        self.assertIn('$Value -cne $Record.Value', text)


if __name__ == '__main__':
    unittest.main()
