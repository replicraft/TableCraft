from pathlib import Path
import tempfile
import unittest

from check_documentation_links import check_documents


class DocumentationLinks(unittest.TestCase):
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
