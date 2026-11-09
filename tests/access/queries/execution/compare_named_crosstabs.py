"""Independent fixed-crosstab dependencies, row grids and buffered refusal checks."""
import argparse
import csv
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES
from compare_saved_forests import PROFILES


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "directory"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
    expected = {
        "CrossFixedParameter": [["Region", "Total", "1", "2", "3"], ["2", "5", "NULL", "NULL", "5"], ["3", "8", "NULL", "NULL", "NULL"]],
        "CrossFixedText": [["Region", "Total", "Sales.Q2", "Sales!Q1"], ["1", "20", "17", "3"], ["2", "5", "NULL", "5"], ["3", "8", "NULL", "8"]],
        "CrossFixedTop": [["Region", "Total", "3", "2", "4"], ["1", "20", "NULL", "17", "NULL"]],
        "CrossFixedTopTies": [["Region", "Total", "3", "2", "4"], ["1", "20", "NULL", "17", "NULL"], ["3", "8", "NULL", "NULL", "NULL"]],
        "NamedFixed": [["Region", "Three", "Two", "Four"], ["1", "NULL", "17", "NULL"], ["2", "5", "NULL", "NULL"], ["3", "NULL", "NULL", "NULL"]],
        "NamedProjection": [["Region"], ["1"]],
        "NamedJoin": [["Region", "Two"], ["1", "17"]],
        "NamedUnion": [["Region"], ["1"], ["2"]],
        "NamedCorrelated": [["Region", "Two"], ["1", "17"], ["2", "NULL"], ["3", "NULL"]],
        "NamedNested": [["Region", "Two"], ["1", "17"]],
        "NamedParameter": [["Region", "Known"], ["2", "5"], ["3", "NULL"]],
        "NamedText": [["Region", "Two"], ["1", "17"], ["2", "NULL"], ["3", "NULL"]],
        "NamedTop": [["Region", "Total", "Three", "Two", "Four"], ["1", "20", "NULL", "17", "NULL"]],
        "NamedTopTies": [["Region"], ["1"], ["3"]],
        "NamedCrosstab": [["Bucket", "1", "2", "4"], ["0", "1", "1", "NULL"]],
    }
    refused = {"CrossFixedError", "CrossFixedCycle", "NamedError", "NamedCycle"}
    controls = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-named-cross-oracle-") as classes:
        sources = [RECIPES / (name + ".java") for name in
                   ("SavedCrosstabFixture", "SavedNamedCrosstabFixture", "SavedReadOracle", "SavedQueryOracle")]
        subprocess.run(["javac", "-cp", str(args.jar), "-d", classes, *map(str, sources)], check=True)

        def java(name, *values):
            return subprocess.check_output([
                "java", "-Dstdout.encoding=UTF-8", "-cp",
                str(args.jar) + os.pathsep + classes, name, *map(str, values)
            ], text=True, encoding="utf-8")

        def cli(*values, accepted=True):
            result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True, text=True)
            if accepted:
                assert result.returncode == 0, (values, result.stderr)
            else:
                assert result.returncode == 2 and not result.stdout, (values, result)
            return result.stdout

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java("SavedNamedCrosstabFixture", args.directory / ("named-cross-" + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ("named-cross-" + suffix)
            before = path.read_bytes()
            names = java("SavedReadOracle", path).splitlines()
            assert len(names) == 33
            names = [name for name in names if name in expected or name in refused]
            assert set(names) == set(expected) | refused
            for name in names:
                raw = java("SavedQueryOracle", path, name)
                report = cli("inspect", path, "--query", name)
                assert list(csv.reader(StringIO(report), delimiter="\t")) == [line.split("\t") for line in raw.splitlines()]
                controls += 1
                sql = java("SavedReadOracle", path, name)
                if name in {"CrossFixedTop", "CrossFixedTopTies"}:
                    flags = [line.split("\t") for line in raw.splitlines()[5:] if line.startswith("3\t")]
                    assert len(flags) == 1 and flags[0][2] == "16" and flags[0][4] == '"1"'
                    assert "TOP" not in sql
                    sql = sql.replace("SELECT ", "SELECT TOP 1 ", 1)
                assert cli("query", "saved", "--database", path, name) == cli("query", "parse", sql), name
                controls += 1
                for query in (sql, None, "SELECT * FROM [" + name + "]"):
                    command = ["query", "run"]
                    command += ["--saved", name] if query is None else [query]
                    command += ["--database", path, "--collation", "ordinal"]
                    if name in {"CrossFixedParameter", "NamedParameter"}:
                        command += ["--param", "Floor=long:2"]
                    output = cli(*command, accepted=name not in refused)
                    if name not in refused:
                        assert list(csv.reader(StringIO(output), delimiter="\t")) == expected[name], (name, output, expected[name])
                    controls += 1
            assert path.read_bytes() == before
            print(path.name, len(before), hashlib.sha256(before).hexdigest())
    print(f"Passed {controls} named fixed-crosstab getter/rendering/execution/refusal controls; native Access unrun.")


if __name__ == "__main__":
    main()
