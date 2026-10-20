"""Compare authored physical text tuples with the reviewed Jackcess reader.

This is independent-reader evidence, not Microsoft Access qualification.
Original databases remain unchanged; mutation checks operate on fresh copies.
"""
import argparse
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import PROFILES, RECIPES, display, run
from compare_large_tables import digest

CASES = [("Pairs", "TextNumber", 1000), ("Pairs", "NumberText", 1000),
         ("Pairs", "MixedDirection", 1000), ("Mixed", "TenAsc", 64),
         ("Mixed", "TenDesc", 64), ("Mixed", "Alternating", 64),
         ("MemoPairs", "MemoTextNumber", 16), ("MemoPairs", "MemoDescending", 16),
         ("UniquePairs", "UniquePair", 5), ("MemoUniquePair", "UniqueMemoPair", 1),
         ("EmptyPairs", "EmptyPair", 0)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "rows", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in PROFILES:
            for version in [0, 1]:
                run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    RECIPES / "TextTupleFixture.java",
                    args.directory / f"TextTuples-{profile}-{version}.{suffix}", profile, version)
    total = 0
    for profile, suffix in PROFILES:
        for version in [0, 1]:
            path = args.directory / f"TextTuples-{profile}-{version}.{suffix}"
            before = digest(path)
            for table, index, count in CASES:
                expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                               RECIPES / "TableDataOracle.java", path, "--date-bits", "--index", table, index)
                lines = [line.split("\t") for line in expected.splitlines()]
                assert len(lines) == count, (profile, version, index, "count")
                actual = run(args.rows, path, table, "--date-bits", "--index", index, "--bytes", 268435456)
                assert actual == expected, (profile, version, index, "exact complete tagged rows")
                for fields, positions in [("ID, Label", [1, 2]), ("ID", [1])]:
                    shown = fields.replace(", ", "\t") + "\n" + "".join("\t".join(display(row[p]) for p in positions) + "\n" for row in lines)
                    assert run(args.cli, "query", "run", f"SELECT {fields} FROM [{table}]", "--database", path,
                               "--index", table, index) == shown, (profile, version, index, fields)
                total += 4  # count, exact complete rows, two projections
            for table in ["UniquePairs", "MemoUniquePair", "EmptyPairs"]:
                report = run(args.cli, "inspect", path, "--table", table, "--check")
                assert ("unsupported" in report.lower()) == (table == "MemoUniquePair"), (profile, version, table, "constraint coverage")
                total += 1
            # Corrupt a selected root on a copy. Both projection shapes must fail
            # with no stdout; neither may fall back to a sequential scan.
            metadata = run(args.cli, "inspect", path, "--table", "Pairs", "--indexes")
            root = next(int(line.split("\t")[3]) for line in metadata.splitlines() if line.startswith("TextNumber\t"))
            image = bytearray(path.read_bytes())
            assert image[root * 4096] == 3, (profile, version, "node root")
            image[root * 4096] = 0
            with tempfile.TemporaryDirectory(prefix="tablecraft-text-tuple-") as temporary:
                damaged = Path(temporary) / ("damaged." + suffix)
                damaged.write_bytes(image)
                damaged_before = digest(damaged)
                for fields in ["ID, Label", "ID"]:
                    refused = subprocess.run([str(args.cli), "query", "run", f"SELECT {fields} FROM Pairs", "--database", str(damaged), "--index", "Pairs", "TextNumber"], capture_output=True)
                    assert refused.returncode == 2 and not refused.stdout, (profile, version, fields, "selected corruption")
                    total += 1
                assert digest(damaged) == damaged_before
            assert digest(path) == before, (profile, version, "source changed")
            total += 2  # node root and source preservation
            print(f"{profile} General {version}: {before}; {path.stat().st_size} bytes; completed {total} controls", flush=True)
    print(f"Passed {total} controls", flush=True)


if __name__ == "__main__":
    main()
