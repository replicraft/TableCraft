"""Compare physical Memo index reads/ranges while preserving full stored values.

The original generator uses the reviewed Jackcess artifact. This is independent
reader/writer evidence, not native Access creation/save or SQL locale evidence.
"""
import argparse
from collections import defaultdict
from pathlib import Path
import subprocess

from compare_browsing import PROFILES, RECIPES, display, run
from compare_large_tables import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "rows", "ranges", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in PROFILES:
            for version in [0, 1]:
                run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "MemoIndexFixture.java", args.directory / f"MemoIndexes-{profile}-{version}.{suffix}", profile, version)
    controls = 0
    for profile, suffix in PROFILES:
        for version in [0, 1]:
            path = args.directory / f"MemoIndexes-{profile}-{version}.{suffix}"
            before = digest(path)
            metadata = run(args.cli, "inspect", path, "--table", "MemoIndexed", "--indexes")
            image = path.read_bytes()
            for index in ["MemoAsc", "MemoDesc"]:
                root = next(int(line.split("\t")[3]) for line in metadata.splitlines() if line.startswith(index + "\t"))
                assert image[root * 4096] == 3, (profile, version, index, "multi-page root")
            for table, index, count in [("MemoIndexed", "MemoAsc", 128), ("MemoIndexed", "MemoDesc", 128), ("MemoChecked", "CheckedMemo", 12), ("MemoEmpty", "EmptyMemo", 0), ("MemoUnique", "UniqueMemo", 1)]:
                expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", path, "--index", table, index)
                lines = [line.split("\t") for line in expected.splitlines()]
                assert len(lines) == count
                assert run(args.rows, path, table, "--index", index, "--bytes", 268435456) == expected, (profile, version, index, "full Memo values")
                shown = "ID\tLabel\n" + "".join("\t".join(display(tag) for tag in row[1:]) + "\n" for row in lines)
                assert run(args.cli, "query", "run", f"SELECT ID, Label FROM [{table}]", "--database", path, "--index", table, index) == shown, (profile, version, index, "full query")
                projected = "ID\n" + "".join(display(row[1]) + "\n" for row in lines)
                assert run(args.cli, "query", "run", f"SELECT ID FROM [{table}]", "--database", path, "--index", table, index) == projected, (profile, version, index, "key projection")
                report = run(args.cli, "inspect", path, "--table", table, "--check")
                if table == "MemoUnique":
                    assert "unsupported" in report.lower(), "Memo prefix equality cannot prove full-value uniqueness"
                else:
                    assert "unsupported" not in report.lower() and "checked" in report.lower()
                controls += 4
            for index in ["MemoAsc", "MemoDesc"]:
                raw = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", path, "--index", "MemoIndexed", index)
                values = [(int(row[1][4:]), None if row[2] == "N" else bytes.fromhex(row[2][1:]).decode("utf-8")) for row in (line.split("\t") for line in raw.splitlines())]
                lengths = "ID\tUnits\n" + "".join(f"{key}\t" + ("NULL" if value is None else str(len(value.encode("utf-16-le")) // 2)) + "\n" for key, value in sorted(values))
                assert run(args.cli, "query", "run", "SELECT ID, Len(Label) AS Units FROM MemoIndexed ORDER BY ID", "--database", path, "--index", "MemoIndexed", index) == lengths
                filtered = "ID\n" + "".join(str(key) + "\n" for key, value in sorted(values) if value is not None and value.endswith("left"))
                assert run(args.cli, "query", "run", "SELECT ID FROM MemoIndexed WHERE Right(Label,4)='left' ORDER BY ID", "--database", path, "--index", "MemoIndexed", index, "--collation", "ordinal") == filtered
                controls += 2
                oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TextRangeOracle.java", path, index, "MemoIndexed")
                expected = defaultdict(list)
                for line in oracle.splitlines():
                    name, row = line.split("\t", 1)
                    expected[name].append(row)
                first, last = ("a", "é") if index == "MemoAsc" else ("é", "a")
                cases = [
                    ("all", ["--mode", "all"]),
                    ("null", ["--mode", "null"]),
                    ("open", []),
                    ("case", ["--start", "A  ", "--end", "a"]),
                    ("empty", ["--start", "a", "--exclude-start", "--end", "A  "]),
                    ("accent", ["--start", "é", "--end", "e\u0301"]),
                    ("interval", ["--start", first, "--end", last]),
                    ("exclusive", ["--start", first, "--exclude-start", "--end", last, "--exclude-end"]),
                    ("start", ["--start", "a"]),
                    ("end", ["--end", "é", "--exclude-end"]),
                    ("reversed", ["--start", last, "--end", first]),
                    ("ignored", ["--start", "", "--end", ""]),
                    ("long", ["--start", "a" * 255, "--end", "a" * 255]),
                    ("supplementary", ["--start", "😀" * 127 + "x", "--end", "😀" * 127 + "x"]),
                ]
                for name, options in cases:
                    full = "".join(row + "\n" for row in expected[name])
                    projected = "".join(row.split("\t")[0] + "\n" for row in expected[name])
                    assert run(args.ranges, path, "--table", "MemoIndexed", "--index", index, *options, "--output-bytes", 2097152) == full, (profile, version, index, name)
                    assert run(args.ranges, path, "--table", "MemoIndexed", "--index", index, *options, "--project-id") == projected
                    controls += 2
            limited = subprocess.run([str(args.ranges), str(path), "--table", "MemoIndexed", "--index", "MemoAsc", "--mode", "all"], capture_output=True)
            assert limited.returncode != 0 and not limited.stdout and b"output limit" in limited.stderr
            controls += 1
            assert digest(path) == before
            print(f"{profile} General {version}: full Memo rows/projections/checks, retained full-value expressions and physical ranges; unchanged {before}; {path.stat().st_size} bytes", flush=True)
    print(controls, "Memo read/query/range/output-limit controls passed; native Access unrun")


if __name__ == "__main__":
    main()
