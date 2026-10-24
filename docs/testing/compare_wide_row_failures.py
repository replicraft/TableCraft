"""Damage fresh copies of registered wide histories and check atomic failures.

Original inputs and their hashes stay unchanged. Independent row locations only
identify the mutation targets; TableCraft alone executes SQL and streaming reads.
"""

import argparse
import csv
from io import StringIO
from pathlib import Path
import struct
import subprocess
import tempfile

from compare_browsing import PROFILES, RECIPES, display
from compare_union_types import JAR_SHA256
from compare_wide_table_histories import INPUT_HASHES, NAMES, NATIVE, digest, model

PAGE_BYTES = 4096
TARGET_ID = 20


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256, "Unreviewed engineering artifact"
    controls = 0

    for profile, suffix in PROFILES:
        path = args.directory / f"wide-{profile}.{suffix}"
        original = path.read_bytes()
        assert digest(path) == INPUT_HASHES[profile], "Unregistered input"
        for table in ["WideVariable", "WideMixed"]:
            locations = subprocess.check_output([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "WideRowLocationOracle.java"), str(path), table,
            ], text=True, encoding="utf-8")
            locations = [tuple(map(int, line.split("\t"))) for line in locations.splitlines()]
            expected = model(table == "WideMixed")
            assert {row[0] for row in locations} == set(expected)
            target = next(row for row in locations if row[0] == TARGET_ID)
            _, page_number, slot, definition = target
            base = page_number * PAGE_BYTES
            assert original[base:base + 2] == b"\x01\x01"
            assert struct.unpack_from("<I", original, base + 4)[0] == definition
            raw = struct.unpack_from("<H", original, base + 14 + 2 * slot)[0]
            assert raw & 0xE000 == 0, "Target must be an ordinary live root"
            start = base + (raw & 0x1FFF)
            end = base + PAGE_BYTES if slot == 0 else base + (
                struct.unpack_from("<H", original, base + 12 + 2 * slot)[0] & 0x1FFF
            )
            assert struct.unpack_from("<H", original, start)[0] == 255
            assert struct.unpack_from("<i", original, start + 2)[0] == TARGET_ID
            count_position = end - 32 - 2
            variables = struct.unpack_from("<H", original, count_position)[0]
            assert variables == (254 if table == "WideVariable" else 51)
            tail_start = count_position - 2 * (variables + 1)
            prefix_ids = [row[0] for row in locations[:locations.index(target)]]
            assert len(prefix_ids) >= 2, "Failure must follow successfully published rows"
            faults = {
                "logical-count": (start, 256),
                "variable-count": (count_position, variables + 1),
                "high-offset": (count_position - 2 * (min(128, variables) + 1), 0),
                "tail-end": (count_position - 2, tail_start - start + 1),
                "first-offset": (count_position - 2 * (variables + 1), 0),
            }
            for name, (offset, value) in faults.items():
                changed = bytearray(original)
                struct.pack_into("<H", changed, offset, value)
                assert changed != original
                with tempfile.TemporaryDirectory(prefix="tablecraft-wide-failure-") as temporary:
                    copy = Path(temporary) / path.name
                    copy.write_bytes(changed)
                    derivative_hash = digest(copy)
                    commands = [
                        ["rows", copy, table, "--limit", "100"],
                        ["query", "run", f"SELECT * FROM {table}", "--database", copy, *NATIVE],
                        ["query", "run", f"SELECT ID FROM {table}", "--database", copy, *NATIVE],
                        ["query", "run", f"SELECT Count(*) FROM {table}", "--database", copy, *NATIVE],
                        ["query", "run", f"SELECT ID FROM {table}", "--database", copy,
                         *NATIVE, "--index", table, "IdAsc"],
                    ]
                    for command in commands:
                        result = subprocess.run(
                            [str(args.cli), *map(str, command)], capture_output=True,
                            text=True, encoding="utf-8",
                        )
                        assert result.returncode == 2 and not result.stdout, (
                            profile, table, name, command[:3], result.stderr,
                        )
                        controls += 1

                    for columns in [[], ["--column", "ID"]]:
                        result = subprocess.run([
                            str(args.cli), "rows", str(copy), table, "--stream", "--limit", "1",
                            *NATIVE, *columns,
                        ], capture_output=True, text=True, encoding="utf-8")
                        assert result.returncode == 2 and result.stderr, (profile, table, name)
                        actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                        selected = [0] if columns else range(255)
                        wanted = [[NAMES[column] for column in selected], *[
                            [display(expected[identity][column]) for column in selected]
                            for identity in prefix_ids
                        ]]
                        assert actual == wanted, (profile, table, name, "published prefix")
                        controls += 1
                    assert digest(copy) == derivative_hash, "Damaged copy changed while reading"
        assert path.read_bytes() == original, "Original changed"
        print(f"{profile}: unchanged SHA-256 {INPUT_HASHES[profile]}", flush=True)
    print(f"Passed {controls} wide-row failure controls; native Access unrun.")


if __name__ == "__main__":
    main()
