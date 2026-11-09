"""Compare explicit physical Text intervals with independent index cursors.

Reuses the original TextIndexFixture corpus. Range semantics are native encoded
start/end order, not SQL text comparison. All mutations affect fresh copies only.
"""
import argparse
from collections import defaultdict
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "ranges", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    controls = 0
    for profile, suffix in PROFILES:
        for version in [0, 1]:
            path = args.directory / f"TextIndexes-{profile}-{version}.{suffix}"
            before = digest(path)
            null_rows = None
            for index in ["TextAsc", "TextDesc"]:
                oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TextRangeOracle.java", path, index)
                expected = defaultdict(list)
                for line in oracle.splitlines():
                    name, row = line.split("\t", 1)
                    expected[name].append(row)
                first, last = ("a", "é") if index == "TextAsc" else ("é", "a")
                cases = [
                    ("all", ["--mode", "all"]), ("null", ["--mode", "null"]), ("open", []),
                    ("case", ["--start", "A  ", "--end", "a"]),
                    ("empty", ["--start", "a", "--exclude-start", "--end", "A  "]),
                    ("accent", ["--start", "é", "--end", "e\u0301"]),
                    ("interval", ["--start", first, "--end", last]),
                    ("exclusive", ["--start", first, "--exclude-start", "--end", last, "--exclude-end"]),
                    ("start", ["--start", "a"]), ("end", ["--end", "é", "--exclude-end"]),
                    ("reversed", ["--start", last, "--end", first]),
                    ("ignored", ["--start", "", "--end", ""]),
                    ("long", ["--start", "a" * 255, "--end", "a" * 255]),
                    ("supplementary", ["--start", "😀" * 127 + "x", "--end", "😀" * 127 + "x"]),
                ]
                for name, options in cases:
                    full = "".join(row + "\n" for row in expected[name])
                    projected = "".join(row.split("\t")[0] + "\n" for row in expected[name])
                    assert run(args.ranges, path, "--index", index, *options) == full, (profile, version, index, name, "rows")
                    assert run(args.ranges, path, "--index", index, *options, "--project-id") == projected, (profile, version, index, name, "projection")
                    controls += 2
                if index == "TextDesc":
                    null_rows = "".join(row + "\n" for row in expected["null"])
            # Descending NULL keys occupy the final physical span. Corrupt the
            # first child of the node root: NULL selection must skip it, while a
            # complete selected-index read must fail with no partial output.
            metadata = run(args.cli, "inspect", path, "--table", "TextIndexed", "--indexes")
            root = next(int(line.split("\t")[3]) for line in metadata.splitlines() if line.startswith("TextDesc\t"))
            image = bytearray(path.read_bytes())
            page = image[root * 4096:(root + 1) * 4096]
            assert page[0] == 3
            end = next(i * 8 + bit for i, byte in enumerate(page[27:480]) for bit in range(8) if byte & (1 << bit))
            child = int.from_bytes(page[480 + end - 4:480 + end], "big")
            assert child != 0 and child != root
            image[child * 4096] = 0
            with tempfile.TemporaryDirectory(prefix="tablecraft-text-range-corrupt-") as directory:
                corrupt = Path(directory) / path.name
                corrupt.write_bytes(image)
                assert run(args.ranges, corrupt, "--index", "TextDesc", "--mode", "null") == null_rows
                failed = subprocess.run([str(args.ranges), str(corrupt), "--index", "TextDesc", "--mode", "all"], capture_output=True)
                assert failed.returncode != 0 and not failed.stdout
                assert corrupt.read_bytes() == image
            for mode in ["all", "null", "range"]:
                failed = subprocess.run([str(args.ranges), str(path), "--index", "PrimaryKey", "--mode", mode], capture_output=True)
                assert failed.returncode != 0 and not failed.stdout
            assert digest(path) == before
            controls += 5
            print(f"{profile} General {version}: 56 cursor/projection selections, 2 pruning/corruption checks, 3 typed refusals; unchanged {before}", flush=True)
    print(controls, "physical Text range controls passed; native Access unrun")


if __name__ == "__main__":
    main()
