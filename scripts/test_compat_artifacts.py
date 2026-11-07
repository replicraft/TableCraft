"""Real temporary-file controls for the shared artifact reader."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from compat_artifacts import ArtifactStore


class ArtifactStoreTests(unittest.TestCase):
    def test_scope_rejects_parent_and_absolute_escapes_without_hiding_missing_files(self):
        with tempfile.TemporaryDirectory() as directory:
            outer = Path(directory).resolve()
            root = outer / 'evidence'
            root.mkdir()
            store = ArtifactStore(root)
            (outer / 'outside.json').write_text('{}')
            self.assertEqual(store.path('./nested/../missing.json'), root / 'missing.json')
            self.assertEqual(store.path(root / 'inside.json'), root / 'inside.json')
            for value in ('../outside.json', outer / 'outside.json'):
                with self.subTest(value=value):
                    with self.assertRaises(ValueError):
                        store.path(value)
                    with self.assertRaises(ValueError):
                        store.pin(value)
                    with self.assertRaises(ValueError):
                        store.load(value)
            with self.assertRaises(FileNotFoundError):
                store.pin('missing.json')
            with self.assertRaises(FileNotFoundError):
                store.load('missing.json')

    def test_symlink_escape_is_refused_when_platform_permits_creation(self):
        with tempfile.TemporaryDirectory() as directory:
            outer = Path(directory).resolve()
            root = outer / 'evidence'
            root.mkdir()
            (outer / 'outside.json').write_text('{}')
            try:
                (root / 'escape').symlink_to(outer, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"Host does not permit creating symlink fixtures: {error}")

            store = ArtifactStore(root)
            for read in (store.path, store.pin, store.load):
                with self.subTest(read=read.__name__):
                    with self.assertRaises(ValueError):
                        read('escape/outside.json')

    def test_json_uses_utf8_instead_of_the_host_legacy_codepage(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / 'facts.json'
            target.write_bytes('{"text":"日本語 · café"}'.encode('utf-8'))
            original_read = Path.read_text

            def legacy_read(path, *args, **options):
                options.setdefault('encoding', 'cp1252')
                return original_read(path, *args, **options)

            with patch.object(Path, 'read_text', legacy_read):
                self.assertEqual(
                    ArtifactStore(root).load('facts.json'),
                    {'text': '日本語 · café'},
                )

    def test_literal_hashes_changed_files_and_json_errors_are_observed_fresh(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            store = ArtifactStore(root)
            payload = root / 'payload.bin'
            payload.write_bytes(b'abc')
            first = store.pin('payload.bin')
            self.assertEqual(first, {
                'path': 'payload.bin',
                'bytes': 3,
                'sha256': 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
            })
            payload.write_bytes(b'abcd')
            self.assertEqual(store.pin(payload), {
                'path': 'payload.bin',
                'bytes': 4,
                'sha256': '88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589',
            })
            self.assertNotEqual(store.pin('payload.bin'), first)
            payload.unlink()
            with self.assertRaises(FileNotFoundError):
                store.pin('payload.bin')
            document = root / 'facts.json'
            document.write_bytes(b'{"items":[1,true,null],"text":"\\u00e9"}')
            self.assertEqual(store.load('facts.json'), {'items': [1, True, None], 'text': 'é'})
            document.write_bytes(b'{"items": [}')
            with self.assertRaises(json.JSONDecodeError):
                store.load('facts.json')


if __name__ == '__main__':
    unittest.main()
