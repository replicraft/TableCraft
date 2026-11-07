from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_documentation_links as guard
from check_documentation_links import check_documents


class DocumentationLinks(unittest.TestCase):
    def test_git_inventory_uses_utf8_for_unicode_document_names(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "日本語.md").write_text("[target](目標.md)\n", encoding="utf-8")
            (root / "目標.md").write_text("# Target\n", encoding="utf-8")
            encoded = "日本語.md\0目標.md\0".encode("utf-8")

            def inventory(command, **options):
                self.assertEqual(options["encoding"], "utf-8")
                return encoded.decode(options["encoding"])

            with patch.object(guard.subprocess, "check_output", side_effect=inventory):
                with patch.object(guard, "print", create=True), patch.object(
                    guard.argparse.ArgumentParser, "parse_args",
                    return_value=type("Arguments", (), {"root": root})(),
                ):
                    with self.assertRaises(SystemExit) as stopped:
                        guard.main()
            self.assertEqual(stopped.exception.code, False)

    def test_wrapped_and_nested_labels_resolve_relocated_documents(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "docs/area").mkdir(parents=True)
            (root / "docs/area/target.md").write_text("# Target\n")
            (root / "docs/index.md").write_text(
                "[wrapped\nlabel](area/target.md#target)\n"
                "[nested [label]](area/target.md)\n"
                "[stale\nlabel](target.md)\n"
            )
            checked, missing = check_documents(root, ["docs/index.md"])
            self.assertEqual(checked, 3)
            self.assertEqual(missing, [("docs/index.md", 5, "target.md")])

    def test_reference_images_html_and_encoded_spaces(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "image file.png").write_bytes(b"original fixture")
            (root / "index.md").write_text(
                "[picture]: <image file.png>\n"
                "![image](image%20file.png)\n"
                '<a href="/image%20file.png">image</a>\n'
                "[^note]: A footnote is not a file target.\n"
            )
            self.assertEqual(check_documents(root, ["index.md"]), (3, []))

    def test_external_urls_fragments_and_removed_sources_are_not_file_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "index.md").write_text(
                "[section](#section) [site](https://example.com/page)\n"
                "[email](mailto:example@example.com)\n"
            )
            self.assertEqual(check_documents(root, ["index.md", "removed.md"]), (0, []))


if __name__ == "__main__":
    unittest.main()
