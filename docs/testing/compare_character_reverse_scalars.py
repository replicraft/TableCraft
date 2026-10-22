"""Independent scalar spellings, ASCII codes and reversal; no Access SQL oracle."""
import argparse
import csv
from decimal import Decimal
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES
from compare_replacement import spelling
from compare_union_types import JAR_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def query(mode, sql, expected, options=(), diagnostic=None):
        nonlocal controls
        result = subprocess.run([str(args.cli), "query", mode, sql, *map(str, options)],
                                text=True, encoding="utf-8", capture_output=True, check=False)
        if diagnostic:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert diagnostic in result.stderr, (sql, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == expected, (sql, rows, expected)
        controls += 1

    for profile in ["en-us", "de-de"]:
        cases = [("True", "True"), ("False", "False")]
        for value in [-(2**63), -(2**53+1), 0, 2**53+1, 2**63-1]:
            cases.append((f"CLngLng({value})", str(value)))
        for quarter in range(-8, 9):
            value = Decimal(quarter) / 4
            invariant = str(value)
            text = invariant.replace(".", ",") if profile == "de-de" else invariant
            for constructor in ["CSng", "CDbl", "CCur", "CDec"]:
                cases.append((f"{constructor}({invariant})", text))
        cases.append(("#1900-01-01#", "1900-01-01"))
        for mode in ["eval", "run"]:
            options = ["--number-format", profile, "--date-format", "iso"]
            for expression, text in cases:
                assert text.isascii() and text
                sql = (f"SELECT Asc({expression}),AscW({expression}),StrReverse({expression}),"
                       f"VarType(Asc({expression})),VarType(AscW({expression})),VarType(StrReverse({expression}))")
                expected = [[str(ord(text[0])), str(ord(text[0])), spelling(text[::-1]), "2", "2", "8"]]
                query(mode, sql, expected, options)
            for function in ["Asc", "AscW", "StrReverse"]:
                query(mode, f"SELECT {function}(NULL)", None, diagnostic="invalid use of NULL in conversion")
            query(mode, "SELECT Asc(123)", None, diagnostic="explicit number-format policy")
            query(mode, "SELECT StrReverse(1/0)", None, diagnostic="division by zero")

    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        output = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows"], text=True, encoding="utf-8")
        tags = [line.split("\t")[1] for line in output.splitlines()]
        assert len(tags) == 20000 and all(tag.startswith("I32:") for tag in tags)
        chosen = sorted(int(tag[4:]) for tag in tags if 12340 <= int(tag[4:]) <= 12344)
        assert len(chosen) == 5
        expected = [[str(value), str(ord(str(value)[0])), str(ord(str(value)[0])), spelling(str(value)[::-1])] for value in chosen]
        options = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
                   "--native-steps", "10000000", "--execution-steps", "10000000", "--number-format", "en-us"]
        sql = "SELECT ID,Asc(ID),AscW(ID),StrReverse(ID) FROM IndexedRows WHERE ID>=12340 AND ID<=12344 ORDER BY ID"
        for index in [None, "IdAsc", "IdDesc"]:
            query("run", sql, expected, [*options, *(["--index", "IndexedRows", index] if index else [])])
        assert path.read_bytes() == before
    print(f"Passed {controls} independent scalar character/reversal controls; native Access unrun.")


if __name__ == "__main__":
    main()
