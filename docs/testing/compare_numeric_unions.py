"""Check provisional numeric UNION carriers with independent exact/IEEE models.

Native stored-value getters are optional; no Access SQL is executed here.
"""
import argparse
import csv
from decimal import Decimal
import hashlib
from io import StringIO
import json
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES
from compare_union_types import JAR_SHA256, NATIVE_HASHES

KINDS = ["Boolean", "Byte", "Integer", "Long", "LongLong", "Single", "Double", "Currency", "Decimal"]
EXPRESSIONS = ["CBool(1)", "CByte(255)", "CInt(-32768)", "CLng(2147483647)",
               "CLngLng(9223372036854775807)", "CSng(0.1)", "CDbl(0.1)",
               "CCur('1.2345')", "CDec('9007199254740993.125')"]
VALUES = [-1, 255, -32768, 2147483647, 9223372036854775807,
          struct.unpack("f", struct.pack("f", 0.1))[0], 0.1, Decimal("1.2345"), Decimal("9007199254740993.125")]
MATRIX = [
    [0, 2, 2, 3, 4, 6, 6, 7, 8],
    [2, 1, 2, 3, 4, 6, 6, 7, 8],
    [2, 2, 2, 3, 4, 6, 6, 7, 8],
    [3, 3, 3, 3, 4, 6, 6, 7, 8],
    [4, 4, 4, 4, 4, 6, 6, 8, 8],
    [6, 6, 6, 6, 6, 5, 6, 6, 6],
    [6, 6, 6, 6, 6, 6, 6, 6, 6],
    [7, 7, 7, 7, 8, 6, 6, 7, 8],
    [8, 8, 8, 8, 8, 6, 6, 8, 8],
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--native-directory", type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.native_directory):
        parser.error("--jar and --native-directory must be supplied together")
    controls = 0

    def check(body, expected, carrier, options=()):
        nonlocal controls
        sql = f"SELECT N,TypeName(N) AS Carrier FROM ({body}) AS U"
        result = subprocess.run([str(args.cli), "query", "run", sql, "--number-format", "invariant", *map(str, options)],
                                text=True, encoding="utf-8", capture_output=True)
        assert result.returncode == 0, (body, result.stderr)
        assert not result.stderr
        rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        assert rows[0] == ["N", "Carrier"]
        actual = []
        for value, kind in rows[1:]:
            expected_kind = "Null" if value == "NULL" else carrier
            assert json.loads(kind) == expected_kind, (body, kind, expected_kind)
            actual.append(None if value == "NULL" else struct.unpack("f", struct.pack("f", float(value)))[0] if carrier == "Single" else float(value) if carrier == "Double"
                          else -1 if value == "TRUE" else 0 if value == "FALSE" else Decimal(value))
        assert actual == expected, (body, actual, expected)
        controls += 1

    for a, left in enumerate(EXPRESSIONS):
        for b, right in enumerate(EXPRESSIONS):
            kind = MATRIX[a][b]
            values = [float(VALUES[a]), float(VALUES[b])] if kind in (5, 6) else [Decimal(VALUES[a]), Decimal(VALUES[b])]
            for all_rows in (False, True):
                expected = values if all_rows or values[0] != values[1] else values[:1]
                check(f"SELECT {left} AS N UNION {'ALL' if all_rows else ''} SELECT {right}", expected, KINDS[kind])
    for all_rows in (False, True):
        for reverse in (False, True):
            arms = ["CLngLng(9007199254740993)", "CDbl(9007199254740992)"]
            if reverse:
                arms.reverse()
            expected = [float(9007199254740992)] * (2 if all_rows else 1)
            check(f"SELECT {arms[0]} AS N UNION {'ALL' if all_rows else ''} SELECT {arms[1]}", expected, "Double")
    check("SELECT CLngLng(9007199254740993) AS N UNION ALL SELECT CDbl(0) WHERE False", [float(9007199254740992)], "Double")
    check("SELECT Null AS N UNION ALL SELECT CCur(1) UNION ALL SELECT CLngLng(9223372036854775807)",
          [None, Decimal(1), Decimal(9223372036854775807)], "Decimal")

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
        for suffix, digest in NATIVE_HASHES.items():
            path = args.native_directory / ("unicode-" + suffix)
            original = path.read_bytes()
            assert hashlib.sha256(original).hexdigest() == digest
            output = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "TableDataOracle.java"), str(path), "AData"], text=True, encoding="utf-8")
            values = [int(line.split("\t")[1].removeprefix("I32:")) for line in output.splitlines()]
            for expression, expected, carrier in [
                ("CDbl(0.5)", [*map(float, values), 0.5], "Double"),
                ("CCur('1.2345')", [*map(Decimal, values), Decimal("1.2345")], "Currency"),
                ("CDec('9007199254740993.125')", [*map(Decimal, values), Decimal("9007199254740993.125")], "Decimal"),
            ]:
                check(f"SELECT ID AS N FROM AData UNION ALL SELECT {expression}", expected, carrier, ["--database", path])
            assert path.read_bytes() == original
    print(f"Passed {controls} independent numeric UNION carrier/value/rounding controls; native Access unrun.")


if __name__ == "__main__":
    main()
