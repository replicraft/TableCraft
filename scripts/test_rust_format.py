"""Formatter orchestration tests use inert Cargo metadata and no Rust processes."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import check_rust_format as formatter


class RustFormatTests(unittest.TestCase):
    def test_all_workspace_target_kinds_keep_editions_configs_and_deduplicate(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            crate = root / "member"
            crate.mkdir()
            (crate / "rustfmt.toml").write_text("max_width = 100\n", encoding="utf-8")
            targets = [
                {"src_path": f"member/{kind}.rs", "edition": "2024", "kind": [kind]}
                for kind in ["lib", "bin", "example", "test", "bench", "custom-build"]
            ]
            targets.append(dict(targets[0]))
            targets.append({"src_path": "legacy/lib.rs", "edition": "2021"})
            metadata = {
                "workspace_members": ["actual-member"],
                "packages": [
                    {"id": "actual-member", "targets": targets},
                    {"id": "foreign-dependency", "targets": [{"src_path": "foreign.rs", "edition": "2018"}]},
                ],
            }
            groups = formatter.target_groups(metadata, root)
            self.assertEqual(len(groups[("2024", str(Path("member/rustfmt.toml")))]), 6)
            self.assertEqual(groups[("2021", None)], {str(Path("legacy/lib.rs"))})
            flattened = [argument for command in formatter.commands(groups) for argument in command]
            self.assertNotIn("foreign.rs", flattened)
            self.assertNotIn("skip_children=true", flattened)
            self.assertNotIn("--all", flattened)

    def test_windows_utf16_batches_cover_all_long_unicode_example_paths_once(self):
        self.assertEqual(formatter.command_units(["🧩"]), 3)
        sources = {f'examples/quoted directory/日本語-🧩-{position:04d}-' + "x" * 180 + ".rs" for position in range(600)}
        commands = list(formatter.commands({("2024", None): sources}))
        self.assertGreater(len(commands), 1)
        actual = []
        for command in commands:
            self.assertEqual(command[:4], ["rustfmt", "--check", "--edition", "2024"])
            self.assertLessEqual(formatter.command_units(command), 16_000)
            actual.extend(command[4:])
        self.assertEqual(sorted(actual), sorted(sources))
        with self.assertRaisesRegex(ValueError, "One formatter target"):
            list(formatter.commands({("2024", None): {"x" * 16_000 + ".rs"}}))

    def test_failed_formatter_stops_with_its_original_status(self):
        metadata = {
            "workspace_members": ["member"],
            "packages": [{"id": "member", "targets": [
                {"src_path": "one/lib.rs", "edition": "2021"},
                {"src_path": "two/lib.rs", "edition": "2024"},
            ]}],
        }
        with tempfile.TemporaryDirectory() as temporary:
            with patch.object(formatter.subprocess, "run", side_effect=[
                subprocess.CompletedProcess([], 0, stdout=json.dumps(metadata)),
                subprocess.CompletedProcess([], 9),
            ]) as run:
                self.assertEqual(formatter.check(Path(temporary)), 9)
                self.assertEqual(run.call_count, 2)
                self.assertEqual(run.call_args_list[0].args[0], [
                    "cargo", "metadata", "--no-deps", "--format-version", "1", "--locked",
                ])

    def test_metadata_failure_retains_nonzero_status_and_diagnostic(self):
        with patch.object(formatter.subprocess, "run", side_effect=subprocess.CalledProcessError(
            7, ["cargo"], stderr="metadata refused",
        )), patch.object(formatter, "print", create=True) as output:
            self.assertEqual(formatter.main(), 7)
            output.assert_called_once()


if __name__ == "__main__":
    unittest.main()
