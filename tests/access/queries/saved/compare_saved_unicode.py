"""Compare original Unicode saved sources with independent rendering and row models.

No native Access SQL or application content executes. Labels use exact Unicode
spelling with ASCII case equivalence; wider native NLS identity is unqualified.
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
from compare_saved_forests import PROFILES

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "directory"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-unicode-sources-") as classes:
        sources = [RECIPES / (name + ".java") for name in
                   ("SavedForestFixture", "SavedUnicodeFixture", "SavedReadOracle", "TableDataOracle")]
        subprocess.run(["javac", "-cp", str(args.jar), "-d", classes, *map(str, sources)], check=True)

        def java(name, *values):
            return subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp",
                str(args.jar) + os.pathsep + classes, name, *map(str, values)], text=True, encoding="utf-8")

        def cli(*values):
            result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True, text=True, encoding="utf-8")
            assert result.returncode == 0, (values, result.stderr)
            return result.stdout

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java("SavedUnicodeFixture", args.directory / ("unicode-" + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ("unicode-" + suffix)
            original = path.read_bytes()
            inputs = {name: [] for name in ("AData", "BData", "左側🦀", "右側")}
            for record in java("TableDataOracle", path, *inputs).splitlines():
                table, value = record.split("\t")
                inputs[table].append(int(value.removeprefix("I32:")))
            assert inputs["左側🦀"] == inputs["AData"] == [1, 2, 4]
            assert inputs["右側"] == inputs["BData"] == [2, 3, 4]
            left, right = inputs["AData"], inputs["BData"]
            outer = [[value, value if value in right else None] for value in left]
            inner = [[value, value] for value in left if value in right]
            product = [[a, b] for a in left for b in right]
            reverse = [[value if value in left else None, value] for value in right]
            expected = {
                "UnicodeAliases": outer, "UnicodeTables": outer,
                "UnicodeQuoted": inner, "UnicodeDerived": inner,
                "UnicodeComma": product, "UnicodeNested": product, "UnicodeScripts": reverse,
            }
            names = java("SavedReadOracle", path).splitlines()
            assert len(names) == 21
            assert {name for name in names if name.startswith("Unicode")} == set(expected)
            for name, rows in expected.items():
                sql = java("SavedReadOracle", path, name)
                assert cli("query", "saved", "--database", path, name) == cli("query", "parse", sql), (path, name)
                controls += 1
                ordered = sorted(rows, key=lambda row: tuple((value is not None, value or 0) for value in row))
                wanted = [["左の値", "右の値"], *[["NULL" if value is None else str(value) for value in row] for row in ordered]]
                for text in (sql, None, f"SELECT * FROM [{name}]"):
                    command = ["query", "run"] + (["--saved", name] if text is None else [text]) + ["--database", path]
                    actual = list(csv.reader(StringIO(cli(*command)), delimiter="\t", strict=True))
                    assert actual == wanted, (path, name, actual, wanted)
                    controls += 1
            assert path.read_bytes() == original
            print(path.name, len(original), hashlib.sha256(original).hexdigest())
    print(f"Passed {controls} Unicode rendering/direct/saved/named controls; native Access unrun.")


if __name__ == "__main__":
    main()
