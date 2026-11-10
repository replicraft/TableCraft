"""Container framing checks are distinct from native installer-tool verification."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import installer_contract as installers
from test_package_cli import native_binary


def minimal_container(target, kind):
    # Deliberately only framing literals, not qualified native install fixtures.
    if kind == "msi":
        return b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1" + bytes(504)
    if kind == "dmg":
        return b"koly" + bytes(508)
    if kind == "deb":
        return b"!<arch>\n"
    if kind == "rpm":
        return b"\xed\xab\xee\xdb" + bytes(92)
    data = bytearray(native_binary(target))
    data[8:11] = b"AI\x02"
    return bytes(data)


class InstallerContractTests(unittest.TestCase):
    def test_exact_requested_ten_installer_names_and_platform_formats(self):
        expected = {
            f"tablecraft-desktop-0.1.0-{platform}-{arch}.{extension}"
            for platform, extensions in (("windows", ("msi",)), ("macos", ("dmg",)),
                                         ("linux", ("deb", "rpm", "AppImage")))
            for arch in ("x64", "arm64") for extension in extensions
        }
        self.assertEqual(installers.names("0.1.0"), expected)
        with self.assertRaises(ValueError):
            installers.name("0.1.0", "aarch64-apple-darwin", "msi")

    def test_framing_rejects_wrong_magic_changed_names_and_symlinks(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = "x86_64-pc-windows-msvc"
            path = root / installers.name("0.1.0", target, "msi")
            path.write_bytes(minimal_container(target, "msi"))
            installers.verify_container(path, "0.1.0", target, "msi")
            path.write_bytes(b"wrong" + bytes(507))
            with self.assertRaises(ValueError):
                installers.verify_container(path, "0.1.0", target, "msi")
            path.unlink()
            source = root / "untrusted"
            source.write_bytes(minimal_container(target, "msi"))
            path.symlink_to(source)
            with self.assertRaises(ValueError):
                installers.verify_container(path, "0.1.0", target, "msi")


if __name__ == "__main__":
    unittest.main()
