"""Independent native-log record examples; these tests do not execute Windows Installer."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import verify_msi_rollback as rollback

PRODUCT = "{12345678-1234-1234-1234-123456789ABC}"
INSTALLATION = r"D:\Fixture\Installed TableCraft"
NAMES = ("tablecraft-desktop.exe", "LICENSE", "ThirdPartyLicenses.txt", "RELEASE.json")


def records():
    lines = ["Executing op: ProductUnpublish(PackageKey={first-party-example})"]
    lines.extend("Executing op: FileRemove(,FileName=" + name + ",,)" for name in NAMES)
    lines.extend([
        "Action ended 7:32:56: RemoveExistingProducts. Return value 1.",
        "Action ended 7:32:56: InstallFinalize. Return value 3.",
        "Executing op: Header(Signature=example,ScriptType=2,ScriptAttributes=1)",
        "Executing op: ProductInfo(ProductKey=" + PRODUCT + ",ProductName=TableCraft)",
    ])
    lines.extend(
        "Executing op: FileCopy(SourceName=D:\\Config.Msi\\example.rbf,,DestName="
        + INSTALLATION + "\\" + name + ",Attributes=32800)"
        for name in NAMES
    )
    lines.append("Action ended 7:32:56: INSTALL. Return value 3.")
    return "\n".join(lines)


class NativeRollbackEvidenceTests(unittest.TestCase):
    def verify(self, text):
        return rollback.verify(text, INSTALLATION, PRODUCT)

    def test_ordered_backup_restoration_proves_all_four_exact_payloads(self):
        evidence = self.verify(records())
        self.assertEqual(evidence["exact_payload_backups_restored"], list(NAMES))
        self.assertTrue(evidence["original_product_restored"])

    def test_preflight_failure_or_policy_mentions_are_not_rollback(self):
        for text in ("DisableRollback is 0\nRemoveExistingProducts\nRollback:",
                     records().replace("Return value 1.", "Return value 0.", 1),
                     records().replace("ScriptType=2,", "ScriptType=1,")):
            with self.subTest(text=text):
                with self.assertRaises(ValueError):
                    self.verify(text)

    def test_every_original_removal_and_backup_restore_is_required(self):
        for name in NAMES:
            with self.subTest(name=name):
                text = records().replace("FileRemove(,FileName=" + name, "Unrelated(,FileName=" + name)
                with self.assertRaises(ValueError):
                    self.verify(text)
                text = records().replace("\\" + name + ",Attributes=32800", "\\wrong-" + name + ",Attributes=32800")
                with self.assertRaises(ValueError):
                    self.verify(text)

    def test_other_product_destination_or_nonbackup_copy_is_refused(self):
        for text in (records().replace(PRODUCT, "{00000000-0000-0000-0000-000000000000}"),
                     records().replace(INSTALLATION, r"D:\Unrelated"),
                     records().replace("example.rbf", "source.cab")):
            with self.subTest(text=text):
                with self.assertRaises(ValueError):
                    self.verify(text)

    def test_reordered_restoration_before_failure_is_refused(self):
        lines = records().splitlines()
        lines[6], lines[-1] = lines[-1], lines[6]
        with self.assertRaises(ValueError):
            self.verify("\n".join(lines))


if __name__ == "__main__":
    unittest.main()
