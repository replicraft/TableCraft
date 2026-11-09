"""Independent native derived-source rendering and explicit expected row checks."""
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
        "DerivedLocal": [["KeyA"], ["2"], ["4"]],
        "DerivedUnion": [["KeyA"], ["1"], ["3"]],
        "DerivedNested": [["KeyA"], ["1"], ["2"]],
        "DerivedAnonymous": [["KeyA"], ["1"], ["2"], ["4"]],
        "DerivedJoin": [["KeyA", "KeyB"], ["1", "NULL"], ["2", "2"]],
        "DerivedAggregate": [["KeyA"], ["3"]],
        "DerivedParameter": [["KeyA"], ["2"], ["4"]],
        "DerivedNamed": [["KeyA"], ["1"], ["2"], ["4"]],
    }
    refused = {"DerivedUdf", "DerivedLateral", "DerivedOwner"}
    controls = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-derived-oracle-") as classes:
        sources = [RECIPES / (name + ".java") for name in
                   ("SavedForestFixture", "SavedDerivedFixture", "SavedReadOracle")]
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
                java("SavedDerivedFixture", args.directory / ("derived-" + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ("derived-" + suffix)
            before = path.read_bytes()
            names = java("SavedReadOracle", path).splitlines()
            assert len(names) == 25
            names = [name for name in names if name.startswith("Derived")]
            assert set(names) == set(expected) | refused
            for name in names:
                sql = java("SavedReadOracle", path, name)
                assert cli("query", "saved", "--database", path, name) == cli("query", "parse", sql)
                controls += 1
                for query in (sql, None, "SELECT * FROM [" + name + "]"):
                    command = ["query", "run"]
                    command += ["--saved", name] if query is None else [query]
                    command += ["--database", path]
                    if name == "DerivedParameter":
                        command += ["--param", "Floor=long:2"]
                    output = cli(*command, accepted=name not in refused)
                    if name not in refused:
                        assert list(csv.reader(StringIO(output), delimiter="\t")) == expected[name], (name, output)
                    controls += 1
            assert path.read_bytes() == before
            print(path.name, len(before), hashlib.sha256(before).hexdigest())
    print(f"Passed {controls} derived-source rendering/execution/refusal controls; native Access unrun.")


if __name__ == "__main__":
    main()
