"""Bounded incremental cleanup fixtures; never touch the real build cache."""
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import prune_ci_incremental as cleaner


TARGET = "aarch64-apple-darwin"


class IncrementalPruneTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tablecraft-incremental-prune-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()

    def put(self, relative, data=b"retained fixture"):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return path

    def test_only_selected_debug_incremental_cache_is_removed(self):
        cache = self.put(f"target/{TARGET}/debug/incremental/crate/session/dep-graph.bin")
        unrelated = [
            self.put(f"target/{TARGET}/debug/deps/libexample.rlib"),
            self.put(f"target/{TARGET}/debug/tablecraft"),
            self.put(f"target/{TARGET}/debug/examples/probe"),
            self.put(f"target/{TARGET}/release/incremental/retained"),
            self.put("target/x86_64-pc-windows-msvc/debug/incremental/retained"),
            self.put("target/debug/incremental/host-tools"),
            self.put("docs/fixture.accdb"),
            self.put("target/doc/index.html"),
        ]
        self.assertTrue(cleaner.prune(TARGET, self.root))
        self.assertFalse(cache.exists())
        self.assertFalse((self.root / f"target/{TARGET}/debug/incremental").exists())
        for path in unrelated:
            self.assertEqual(path.read_bytes(), b"retained fixture")
        self.assertFalse(cleaner.prune(TARGET, self.root))

    def test_invalid_or_multiple_targets_refuse_before_any_deletion(self):
        retained = self.put(f"target/{TARGET}/debug/incremental/retained")
        for target in [
            None, "", ".", "..", "../outside", "/tmp", "C:\\outside",
            "a-b-c/../../outside", "a-b-c,d-e-f", "a-b-c d-e-f", "a-b-c\0",
            "a-b-c.json",
        ]:
            with self.subTest(target=target), self.assertRaises(ValueError):
                cleaner.prune(target, self.root)
            self.assertEqual(retained.read_bytes(), b"retained fixture")

    def test_each_symlink_parent_and_the_cache_itself_refuse(self):
        for depth in range(4):
            with self.subTest(depth=depth), tempfile.TemporaryDirectory() as temporary:
                workspace = Path(temporary).resolve()
                outside = self.root / f"outside-{depth}"
                outside.mkdir()
                marker = outside / "must-remain"
                marker.write_bytes(b"outside content")
                parent = workspace
                components = ["target", TARGET, "debug", "incremental"]
                for component in components[:depth]:
                    parent = parent / component
                    parent.mkdir()
                link = parent / components[depth]
                try:
                    link.symlink_to(outside, target_is_directory=True)
                except OSError as error:
                    self.skipTest(f"Host does not permit creating symlink fixtures: {error}")
                with self.assertRaises(ValueError):
                    cleaner.prune(TARGET, workspace)
                self.assertEqual(marker.read_bytes(), b"outside content")
                self.assertTrue(link.is_symlink())

    def test_symlink_inside_cache_is_unlinked_without_following_outside(self):
        marker = self.put("fixture/original.accdb", b"original database")
        cache = self.root / f"target/{TARGET}/debug/incremental"
        cache.mkdir(parents=True)
        try:
            (cache / "foreign-fixture").symlink_to(marker)
        except OSError as error:
            self.skipTest(f"Host does not permit creating symlink fixtures: {error}")
        self.assertTrue(cleaner.prune(TARGET, self.root))
        self.assertEqual(marker.read_bytes(), b"original database")

    def test_nondirectory_parent_refuses_and_missing_parent_is_noop(self):
        self.assertFalse(cleaner.prune(TARGET, self.root))
        target = self.put("target")
        with self.assertRaises(ValueError):
            cleaner.prune(TARGET, self.root)
        self.assertEqual(target.read_bytes(), b"retained fixture")

    def test_entrypoint_rejects_custom_cargo_roots_without_calling_prune(self):
        for name in cleaner.CUSTOM_ROOT_VARIABLES:
            environment = {name: "outside", "CARGO_BUILD_TARGET": TARGET}
            with (
                self.subTest(variable=name),
                patch.dict(os.environ, environment, clear=True),
                patch.object(cleaner, "prune") as prune,
                patch.object(cleaner, "print", create=True),
            ):
                self.assertEqual(cleaner.main(), 1)
                prune.assert_not_called()


if __name__ == "__main__":
    unittest.main()
