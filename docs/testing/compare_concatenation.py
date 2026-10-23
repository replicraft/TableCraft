"""Check contextual & against original Python numeric/date/string models.

Read-only native getters are optional; this does not execute SQL in Access.
"""
import argparse
import csv
import datetime as dt
from decimal import Decimal
import hashlib
from io import StringIO
import json
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES
from compare_cstr import date_text
from compare_union_types import JAR_SHA256, NATIVE_HASHES
from compare_large_indexes import (
    INPUT_HASHES as INDEX_HASHES,
    registered_input_hash,
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--native-directory", type=Path)
    parser.add_argument("--indexed-directory", type=Path)
    parser.add_argument("--jar", type=Path)
    args = parser.parse_args()
    if bool(args.native_directory or args.indexed_directory) != bool(args.jar):
        parser.error("Native directories require --jar; --jar requires a native directory")
    controls = 0

    def check(sql, expected, options=(), commands=("eval", "run"), accepted=True):
        nonlocal controls
        for command in commands:
            result = subprocess.run([str(args.cli), "query", command, sql, *map(str, options)],
                                    capture_output=True, text=True, encoding="utf-8")
            assert result.returncode == (0 if accepted else 2), (sql, result.stderr)
            if accepted:
                assert not result.stderr, result.stderr
                rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                assert rows[0] == ["Result"], rows[0]
                values = [None if row[0] == "NULL" else json.loads(row[0]) for row in rows[1:]]
                assert values == expected, (sql, values, expected)
            else:
                assert not result.stdout, (sql, result.stdout)
            controls += 1

    rng = random.Random(0xC04CA7)
    integers = [-9223372036854775808, -9007199254740993, -1, 0, 1, 9007199254740993, 9223372036854775807]
    integers += [rng.randint(-(2**63), 2**63 - 1) for _ in range(20)]
    for value in integers:
        for expression, expected in [(f"'n=' & CLngLng({value})", "n=" + str(value)),
                                     (f"CLngLng({value}) & Null", str(value))]:
            check("SELECT " + expression + " AS Result", [expected], ["--number-format", "invariant"])
    for coefficient in [-999999999999999999, -12345, 0, 1, 12345, 999999999999999999]:
        decimal = Decimal(coefficient).scaleb(-4)
        spelling = format(decimal, "f").rstrip("0").rstrip(".") if decimal else "0"
        for profile in ("invariant", "de-de"):
            expected = spelling.replace(".", ",") if profile == "de-de" else spelling
            # Exact lexical Decimal input avoids the separate floating literal
            # carrier policy; the input and output explicitly share this format.
            source = format(decimal, "f").replace(".", ",") if profile == "de-de" else format(decimal, "f")
            check(f"SELECT '[' & CDec('{source}') & ']' AS Result", ["[" + expected + "]"],
                  ["--number-format", profile])
    for date in [dt.datetime(1899, 12, 30), dt.datetime(2024, 1, 2), dt.datetime(2024, 1, 2, 13, 4, 5)]:
        for profile in ("iso", "en-us", "de-de"):
            literal = date.strftime("%Y-%m-%d %H:%M:%S")
            check(f"SELECT #{literal}# & '!' AS Result", [date_text(date, profile) + "!"], ["--date-format", profile])
    for expression, expected in [("True & False", "TrueFalse"), ("Null & Null", None),
                                 ("Null & '雪🦀'", "雪🦀"), ("'a' & ''", "a"), ("'a'+Null", None)]:
        check("SELECT " + expression + " AS Result", [expected])
    for expression in ("1 & 'x'", "#2024-01-02# & 'x'", "Null & (1/0)",
                       "IIf(False,1 & 'x','ok')", "CVErr(1) & 'x'"):
        check("SELECT " + expression + " AS Result", None, accepted=False)

    if args.native_directory:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
        for suffix, digest in NATIVE_HASHES.items():
            path = args.native_directory / ("unicode-" + suffix)
            original = path.read_bytes()
            assert hashlib.sha256(original).hexdigest() == digest
            stored = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "TableDataOracle.java"), str(path), "AData"], text=True, encoding="utf-8")
            identifiers = [int(line.split("\t")[1].removeprefix("I32:")) for line in stored.splitlines()]
            expected = ["id=" + str(identifier) for identifier in identifiers]
            for sql in ("SELECT 'id=' & ID AS Result FROM AData",
                        "SELECT 'id=' & ID AS Result FROM (SELECT ID FROM AData) AS Q"):
                check(sql, expected, ["--database", path, "--number-format", "invariant"], commands=("run",))
            check("SELECT 'id=' & ID AS Result FROM AData", None,
                  ["--database", path], commands=("run",), accepted=False)
            assert path.read_bytes() == original
    if args.indexed_directory:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
        for suffix, digest in INDEX_HASHES.items():
            path = args.indexed_directory / ("index-" + suffix)
            digest = registered_input_hash(path, digest)
            original = path.read_bytes()
            assert hashlib.sha256(original).hexdigest() == digest
            for index in ("IdAsc", "IdDesc"):
                stored = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                    str(RECIPES / "IndexRowsOracle.java"), str(path), "IndexedRows", index, "ID"], text=True, encoding="utf-8")
                expected = [line + "!" for line in stored.splitlines() if int(line) >= 19998]
                check("SELECT ID & '!' AS Result FROM IndexedRows WHERE ID>=19998", expected,
                      ["--database", path, "--index", "IndexedRows", index, "--number-format", "invariant"], commands=("run",))
            assert path.read_bytes() == original
    print(f"Passed {controls} contextual concatenation/null/error/native-row controls; native Access unrun.")


if __name__ == "__main__":
    main()
