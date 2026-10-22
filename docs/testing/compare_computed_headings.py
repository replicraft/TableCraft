"""Independent grids and inert metadata checks for computed named fixed crosstabs.

Original authored definitions use the existing original Sales rows. Jackcess renders
metadata and reads fields; it never evaluates SQL. Expected grids are explicit.
"""

import argparse
import csv
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES
from compare_cverr import JAR_SHA256
from compare_saved_forests import PROFILES

INTEGER_GRID = [["Region", "Total", "2", "1", "4"],
                ["1", "20", "17", "3", "NULL"],
                ["2", "5", "NULL", "NULL", "NULL"],
                ["3", "8", "NULL", "NULL", "NULL"]]
TEXT_GRID = [["Region", "Total", "Sales.Q1", "Sales!Q2"],
             ["1", "20", "3", "17"], ["2", "5", "NULL", "5"],
             ["3", "8", "NULL", "8"]]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    expected = {"ComputedFixed": INTEGER_GRID, "ComputedCasts": INTEGER_GRID,
                "ComputedText": TEXT_GRID,
                "ComputedSelect": [["Region", "Two"], ["1", "17"], ["2", "NULL"], ["3", "NULL"]],
                "ComputedTextSelect": [["Region", "Two"], ["1", "17"], ["2", "5"], ["3", "8"]]}
    refused = {"ComputedError", "ComputedConflict", "ComputedLateError"}
    count = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-computed-headings-") as classes:
        names = ["SavedCrosstabFixture", "SavedNamedCrosstabFixture", "ComputedCrosstabFixture",
                 "SavedReadOracle", "SavedQueryOracle", "TableDataOracle"]
        subprocess.run(["javac", "-cp", str(args.jar), "-d", classes,
                        *[str(RECIPES / (name + ".java")) for name in names]], check=True)

        def java(name, *values):
            return subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp",
                                            str(args.jar) + os.pathsep + classes,
                                            name, *map(str, values)], text=True)

        def cli(*values, success=True):
            result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True, text=True)
            if success:
                assert result.returncode == 0, (values, result.stderr)
            else:
                assert result.returncode != 0 and not result.stdout, (values, result.stdout, result.stderr)
            return result.stdout

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java("ComputedCrosstabFixture", args.directory / ("computed-" + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ("computed-" + suffix)
            before = path.read_bytes()
            rows = java("TableDataOracle", path, "Sales").splitlines()
            reference_rows = [["I32:1", "I32:2", "I32:10"], ["I32:1", "I32:1", "I32:3"],
                              ["I32:1", "I32:2", "I32:7"], ["I32:2", "I32:3", "I32:5"],
                              ["I32:2", "I32:1", "N"], ["I32:3", "N", "I32:8"]]
            assert [row.split("\t")[1:] for row in rows] == reference_rows
            count += len(rows)
            for name in sorted(set(expected) | refused | {"ComputedContext"}):
                sql = java("SavedReadOracle", path, name)
                # Reconstruction/inspection remains inert, even for 1/0 headings.
                assert cli("query", "saved", "--database", path, name) == cli("query", "parse", sql)
                raw = java("SavedQueryOracle", path, name)
                report = cli("inspect", path, "--query", name)
                assert list(csv.reader(StringIO(report), delimiter="\t")) == [line.split("\t") for line in raw.splitlines()]
                count += 2
                if name == "ComputedContext":
                    cli("query", "run", "SELECT * FROM ComputedContext", "--database", path,
                        "--number-format", "en-us", success=False)
                    grid = cli("query", "run", "--saved", name, "--database", path,
                               "--number-format", "en-us")
                    assert list(csv.reader(StringIO(grid), delimiter="\t")) == [
                        ["Region", "Total", "2"], ["1", "20", "17"], ["2", "5", "NULL"], ["3", "8", "NULL"]]
                    count += 2
                    continue
                for query in [sql, None, f"SELECT * FROM [{name}]"]:
                    command = ["query", "run"]
                    command += ["--saved", name] if query is None else [query]
                    command += ["--database", path, "--collation", "ordinal"]
                    result = cli(*command, success=name not in refused)
                    if name in expected:
                        assert list(csv.reader(StringIO(result), delimiter="\t")) == expected[name], (name, result)
                    count += 1
            cli("query", "run", "SELECT Region FROM ComputedLateError WHERE Region=1",
                "--database", path, success=False)
            count += 1
            assert path.read_bytes() == before
            print(path.name, len(before), hashlib.sha256(before).hexdigest())
    print(f"Passed {count} independent computed-heading/native-grid/inert/refusal controls; native Access unrun.")


if __name__ == "__main__":
    main()
