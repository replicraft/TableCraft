"""Original saved-query metadata and partition models; no native Access SQL execution."""
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


def verify(output, headers, expected):
    rows = list(csv.reader(StringIO(output), delimiter="\t", strict=True))
    assert rows[0] == headers, rows
    actual = [[None if value == "NULL" else int(value) for value in row] for row in rows[1:]]
    assert actual == expected, (actual, expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
    controls = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-grouped-oracle-") as classes:
        sources = ["SavedForestFixture", "SavedGroupedFixture", "SavedReadOracle", "SavedQueryOracle", "TableDataOracle"]
        subprocess.run(["javac", "-encoding", "UTF-8", "-cp", str(args.jar), "-d", classes,
                        *[str(RECIPES / (name + ".java")) for name in sources]], check=True)

        def java(name, *values):
            return subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp",
                                            str(args.jar) + os.pathsep + classes, name,
                                            *map(str, values)], text=True, encoding="utf-8")

        def cli(*values, accepted=True, error=None):
            result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True,
                                    text=True, encoding="utf-8")
            if accepted:
                assert result.returncode == 0, (values, result.stderr)
            else:
                assert result.returncode == 2 and not result.stdout, (values, result)
                if error is not None:
                    assert error in result.stderr, (values, result.stderr, error)
            return result.stdout

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java("SavedGroupedFixture", args.directory / ("grouped-" + suffix), profile)

        for _, suffix in PROFILES:
            path = args.directory / ("grouped-" + suffix)
            original = path.read_bytes()
            records = java("TableDataOracle", path, "AData", "BData", "CData").splitlines()
            inputs = {name: [] for name in ["AData", "BData", "CData"]}
            for record in records:
                table, value = record.split("\t")
                inputs[table].append(int(value.removeprefix("I32:")))
            assert inputs == {"AData": [1, 2, 4], "BData": [2, 3, 4], "CData": [1, 2, 3]}

            def groups(divisor):
                return [[key, sum(value % divisor == key for value in inputs["AData"])]
                        for key in sorted({value % divisor for value in inputs["AData"]})]

            base = groups(2)
            intersection = set(inputs["BData"]) & set(inputs["CData"])
            expected = {
                "GroupedScalar": base, "GroupedNamed": base, "GroupedDerived": base,
                "GroupedParams": base, "GroupedIn": base, "GroupedAll": base, "GroupedUnion": base,
                "GroupedHaving": [row for row in base if any(value >= 2 and value % 2 == row[0] for value in inputs["AData"])],
                "GroupedNested": [[key, key] for key, _ in base],
                "GroupedJoin": [[key, sum(value % 2 == key for value in intersection)] for key, _ in base],
                "GroupedCrosstab": [[0, 2, None, 1, 1]],
            }
            refused = {
                "GroupedCardinality": "scalar subquery returned multiple rows",
                "GroupedBadCapture": "invalid grouped expression",
                "GroupedBadJoin": "invalid grouped expression",
            }
            names = java("SavedReadOracle", path).splitlines()
            assert len(names) == 28
            names = [name for name in names if name.startswith("Grouped")]
            assert set(names) == set(expected) | set(refused)
            for name in names:
                raw = java("SavedQueryOracle", path, name)
                inspected = cli("inspect", path, "--query", name)
                assert list(csv.reader(StringIO(inspected), delimiter="\t", strict=True)) == [line.split("\t") for line in raw.splitlines()]
                sql = java("SavedReadOracle", path, name)
                assert cli("query", "saved", "--database", path, name) == cli("query", "parse", sql)
                controls += 2
                for divisor in ([2, 3] if name == "GroupedParams" else [2]):
                    for query in [sql, None, "SELECT * FROM [" + name + "]"]:
                        command = ["query", "run"] + (["--saved", name] if query is None else [query]) + ["--database", path]
                        if name == "GroupedParams":
                            command += ["--param", f"Divisor=long:{divisor}"]
                        result = cli(*command, accepted=name not in refused, error=refused.get(name))
                        if name not in refused:
                            headers = ["K", "Total", "1", "2", "4"] if name == "GroupedCrosstab" else ["K", "N"]
                            verify(result, headers, groups(divisor) if name == "GroupedParams" else expected[name])
                        controls += 1
                if name == "GroupedParams":
                    for query in [sql, None, "SELECT * FROM [" + name + "]"]:
                        for invalid, error in [
                            ([], "query parameter mismatch"),
                            (["--param", "Divisor=text:two"], "query parameter mismatch"),
                            (["--param", "Divisor=long:0"], "division by zero"),
                        ]:
                            command = ["query", "run"] + (["--saved", name] if query is None else [query]) + ["--database", path]
                            cli(*command, *invalid, accepted=False, error=error)
                            controls += 1
            assert path.read_bytes() == original
            print(path.name, len(original), hashlib.sha256(original).hexdigest(), flush=True)
    print(f"Passed {controls} grouped-query getter/rendering/direct/saved/named/refusal controls; native Access unrun.")


if __name__ == "__main__":
    main()
