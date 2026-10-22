"""Independent native grids for explicit named-crosstab execution staging.

Jackcess supplies inert metadata rendering and raw row values, never SQL execution.
Expected grids are authored independently; all database bytes must stay unchanged.
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

DYNAMIC = [["Region", "Total", "1", "2", "3"], ["1", "20", "3", "17", "NULL"],
           ["2", "5", "NULL", "NULL", "5"], ["3", "8", "NULL", "NULL", "NULL"]]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "directory"]:
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    count = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-discovery-") as classes:
        names = ["SavedCrosstabFixture", "SavedNamedCrosstabFixture", "ComputedCrosstabFixture",
                 "DiscoveryCrosstabFixture", "SavedReadOracle", "SavedQueryOracle", "TableDataOracle"]
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
            return list(csv.reader(StringIO(result.stdout), delimiter="\t"))

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java("DiscoveryCrosstabFixture", args.directory / ("discovery-" + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ("discovery-" + suffix)
            before = path.read_bytes()
            rows = java("TableDataOracle", path, "Sales").splitlines()
            assert len(rows) == 6
            for name, expected in [("CrossDynamic", DYNAMIC), ("ComputedContext", [
                ["Region", "Total", "2"], ["1", "20", "17"], ["2", "5", "NULL"], ["3", "8", "NULL"]])]:
                sql = java("SavedReadOracle", path, name)
                for query in [sql, None, f"SELECT * FROM [{name}]"]:
                    command = ["query", "run"]
                    command += ["--saved", name] if query is None else [query]
                    command += ["--database", path, "--number-format", "en-us"]
                    assert cli(*command) == expected
                    count += 1
            cases = [
                ("SELECT A.Region,A.[2],B.[3] FROM CrossDynamic A INNER JOIN CrossDynamic B ON A.Region=B.Region ORDER BY A.Region", [["Region", "2", "3"], ["1", "17", "NULL"], ["2", "NULL", "5"], ["3", "NULL", "NULL"]]),
                ("SELECT Region,[1] FROM DiscoveredNested ORDER BY Region", [["Region", "1"], ["1", "17"], ["2", "NULL"], ["3", "NULL"]]),
                ("SELECT Region,IsError(P.[2]) AS Problem FROM DiscoveredErrors P ORDER BY Region", [["Region", "Problem"], ["1", "TRUE"], ["2", "FALSE"], ["3", "FALSE"]]),
                ("SELECT A.[2]=B.[2] AS Same FROM DiscoveredVolatile A INNER JOIN DiscoveredVolatile B ON A.Region=B.Region WHERE A.Region=1", [["Same"], ["TRUE"]]),
            ]
            cases += [
                ("SELECT Region,DSum('[2]','CrossDynamic') AS Total,DCount('*','CrossDynamic','Region>1') AS Other FROM CrossDynamic ORDER BY Region",
                 [["Region", "Total", "Other"], ["1", "17", "2"], ["2", "17", "2"], ["3", "17", "2"]]),
                ("SELECT P.[2]=DLookup('[2]','DiscoveredVolatile','Region=1') AS Same FROM DiscoveredVolatile P WHERE P.Region=1",
                 [["Same"], ["TRUE"]]),
                ("SELECT IsError(DLookup('[2]','DiscoveredErrors','Region=1')) AS Problem FROM DiscoveredErrors WHERE Region=1",
                 [["Problem"], ["TRUE"]]),
            ]
            for sql, expected in cases:
                assert cli("query", "run", sql, "--database", path) == expected
                count += 1
            for profile, value, heading, first in [("en-us", "2.5", "2", "17"), ("de-de", "3,5", "4", "NULL")]:
                expected = [["Region", "Total", heading], ["1", "20", first], ["2", "5", "NULL"], ["3", "8", "NULL"]]
                assert cli("query", "run", "SELECT * FROM DiscoveredParameter ORDER BY Region", "--database", path,
                           "--param", "P=text:" + value, "--number-format", profile) == expected
                count += 1
            expected = [["Region", "Total", "1", "2", "3"], ["2", "5", "NULL", "NULL", "5"], ["3", "8", "NULL", "NULL", "NULL"]]
            assert cli("query", "run", "SELECT * FROM CrossParameter ORDER BY Region", "--database", path, "--param", "Floor=long:2") == expected
            count += 1
            for name in ["DiscoveredParameter", "DiscoveredCycle", "ComputedError", "ComputedLateError"]:
                sql = java("SavedReadOracle", path, name)
                # Both paths remain inert and compare to independent metadata rendering.
                assert cli("query", "saved", "--database", path, name) == cli("query", "parse", sql)
                raw = java("SavedQueryOracle", path, name)
                assert cli("inspect", path, "--query", name) == [line.split("\t") for line in raw.splitlines()]
                count += 2
            for sql in ["SELECT * FROM DiscoveredCycle", "SELECT * FROM DiscoveredParameter",
                        "SELECT Region FROM ComputedLateError WHERE Region=1", "SELECT Region FROM ComputedError"]:
                cli("query", "run", sql, "--database", path, success=False)
                count += 1
            assert path.read_bytes() == before
            print(path.name, len(before), hashlib.sha256(before).hexdigest())
    print(f"Passed {count} independent staged-schema/native-grid/inert/refusal controls; native Access unrun.")


if __name__ == "__main__":
    main()
