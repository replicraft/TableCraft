"""Portable text generation checks; no Cargo, external tools or native database."""
import hashlib
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import sys
import tempfile
import unittest
from unittest.mock import patch

import generate_error_messages as messages
import regenerate_french_like as french
import regenerate_general_like as general


class GeneratorEncodingTests(unittest.TestCase):
    def test_diagnostic_catalog_uses_utf8_and_hashes_the_exact_lf_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            table = root / "table.md"
            table.write_text("|100|[message](example.md)|\n", encoding="utf-8")
            runtime = root / "runtime.resx"
            runtime.write_bytes(b'<root><data name="ID100"/></root>')
            topics = root / "topics"
            topics.mkdir()
            (topics / "example.md").write_text(
                "title: Café · 日本語 (Error 100)\n", encoding="utf-8",
            )
            output = root / "generated"
            arguments = SimpleNamespace(
                table=table, runtime=runtime, topics=topics, output=output,
            )
            original_read = Path.read_text
            original_write = Path.write_text

            def legacy_read(path, *args, **options):
                options.setdefault("encoding", "cp1252")
                return original_read(path, *args, **options)

            def legacy_write(path, *args, **options):
                options.setdefault("encoding", "cp1252")
                options.setdefault("newline", "\r\n")
                return original_write(path, *args, **options)

            with patch.object(Path, "read_text", legacy_read), patch.object(
                Path, "write_text", legacy_write,
            ), patch.object(
                messages.argparse.ArgumentParser, "parse_args", return_value=arguments,
            ), patch.object(messages, "print", create=True):
                messages.main()

            raw = (output / "en-US.ftl").read_bytes()
            self.assertIn("error-100 = Café · 日本語\n".encode("utf-8"), raw)
            self.assertNotIn(b"\r", raw)
            provenance = json.loads((output / "provenance.json").read_bytes())
            self.assertEqual(provenance["catalog_sha256"], hashlib.sha256(raw).hexdigest())
            self.assertNotIn(b"\r", (output / "provenance.json").read_bytes())

    def test_node_insert_recipe_uses_utf8_without_changing_original_inputs(self):
        source = Path(__file__).parent / "compatibility/genuine_node_insert/create_expected.py"
        spec = importlib.util.spec_from_file_location("portable_insert_recipe", source)
        recipe = importlib.util.module_from_spec(spec)
        observation = SimpleNamespace()

        def construct(before, profile):
            self.assertEqual(before, b"independent synthetic original")
            self.assertEqual(profile, {"header": 5, "label": "日本語"})
            return b"independent synthetic expected", {"label": profile["label"]}

        observation.construct = construct
        with patch.dict(sys.modules, {"model": observation}):
            spec.loader.exec_module(recipe)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            request = root / "requests.json"
            request.write_bytes('{"profiles":[{"header":5,"label":"日本語"}]}'.encode("utf-8"))
            original = root / "h5-node-index-original.accdb"
            original.write_bytes(b"independent synthetic original")
            arguments = SimpleNamespace(originals=root, output=root / "generated")
            original_read = Path.read_text

            def legacy_read(path, *args, **options):
                options.setdefault("encoding", "cp1252")
                return original_read(path, *args, **options)

            original_request = request.read_bytes()
            with patch.object(recipe, "HERE", root), patch.object(
                recipe.argparse.ArgumentParser, "parse_args", return_value=arguments,
            ), patch.object(Path, "read_text", legacy_read):
                recipe.main()

            self.assertEqual(request.read_bytes(), original_request)
            self.assertEqual(original.read_bytes(), b"independent synthetic original")
            self.assertEqual(
                (arguments.output / "h5-retained-node-insert-after.accdb").read_bytes(),
                b"independent synthetic expected",
            )

    def test_retained_french_output_is_decoded_as_utf8_without_cargo(self):
        with tempfile.TemporaryDirectory() as directory:
            captured = Path(directory) / "stdout.txt"
            captured.write_bytes("日本語 source\nFRENCH_LIKE_FST 00ff\n".encode("utf-8"))
            arguments = SimpleNamespace(write=False, captured_output=captured)
            original_read = Path.read_text

            def legacy_read(path, *args, **options):
                options.setdefault("encoding", "cp1252")
                return original_read(path, *args, **options)

            with patch.object(Path, "read_text", legacy_read), patch.object(
                Path, "read_bytes", return_value=b"\0\xff",
            ), patch.object(
                french.argparse.ArgumentParser, "parse_args", return_value=arguments,
            ), patch.object(french, "print", create=True), patch.object(
                french.subprocess, "run",
            ) as process:
                french.main()
            process.assert_not_called()

    def test_live_generator_process_contracts_decode_utf8_without_running_cargo(self):
        cases = [
            (
                french,
                SimpleNamespace(write=False, captured_output=None),
                "FRENCH_LIKE_FST 00ff\n",
            ),
            (
                general,
                SimpleNamespace(write=False),
                "GENERAL_LIKE_FST Legacy 00ff\nGENERAL_LIKE_FST General2010 00ff\n",
            ),
        ]
        for generator, arguments, output in cases:
            with self.subTest(generator=generator.__name__):
                def process(command, **options):
                    self.assertEqual(options["encoding"], "utf-8")
                    return SimpleNamespace(stdout=output, stderr="")

                with patch.object(
                    generator.argparse.ArgumentParser, "parse_args", return_value=arguments,
                ), patch.object(
                    generator.subprocess, "run", side_effect=process,
                ), patch.object(
                    Path, "read_bytes", return_value=b"\0\xff",
                ), patch.object(generator, "print", create=True):
                    generator.main()


if __name__ == "__main__":
    unittest.main()
