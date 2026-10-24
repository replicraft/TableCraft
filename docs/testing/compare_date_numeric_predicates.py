"""Independent Date/numeric predicate models for ordinary query execution.

Calendar day/fraction tuples and Decimal raw serials are distinct models.
Only TableCraft executes SQL; native Access qualification remains pending.
"""
import argparse
import csv
from decimal import Decimal
import hashlib
from io import StringIO
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256


def temporal(serial):
    day = int(serial)
    return day, abs(serial - day)


def relations(left, right):
    return [left == right, left != right, left < right, left <= right,
            left > right, left >= right]


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
                                capture_output=True, text=True, encoding="utf-8", check=False)
        if diagnostic:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert diagnostic in result.stderr, (sql, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == expected, (sql, rows, expected)
        controls += 1

    casts = [("CByte", ["0", "1"]), ("CInt", ["-1", "1"]),
             ("CLng", ["-657434", "2958465"]), ("CLngLng", ["-1", "1"]),
             ("CSng", ["-1.25", ".25"]), ("CDbl", ["-.25", "1.25"]),
             ("CCur", ["-1.75", ".25"]), ("CBool", ["-1", "0"]),
             ("CDec", ["-.25", ".25", "2958466"])]
    operators = ["=", "<>", "<", "<=", ">", ">="]
    for mode in ["eval", "run"]:
        for serial_text in ["-1.75", "-.25", ".25", "1.25"]:
            serial = Decimal(serial_text)
            for cast, numbers in casts:
                for number_text in numbers:
                    number = Decimal(number_text)
                    left, right = ((serial, number) if cast == "CDec" else
                                   (temporal(serial), temporal(number)))
                    a, b = f"CDate({serial_text})", f"{cast}({number_text})"
                    expressions = [f"{a}{operator}{b}" for operator in operators]
                    expressions += [f"{b}{operator}{a}" for operator in operators]
                    expected = relations(left, right) + relations(right, left)
                    query(mode, "SELECT " + ",".join(expressions),
                          [["TRUE" if truth else "FALSE" for truth in expected]])
        for number in [-657435, 2958466]:
            query(mode, f"SELECT 1,CDate(0)<CLng({number})", None, diagnostic="numeric overflow")
            query(mode, f"SELECT 1,CLng({number})>CDate(0)", None, diagnostic="numeric overflow")
        query(mode, "SELECT CDate(0)=NULL", [["NULL"]])
        query(mode, "SELECT CDate(0) IN (0,1/0)", None, diagnostic="division by zero")

    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        digest = registered_input_hash(path, digest)
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        output = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "--date-bits", "IndexedRows",
        ], text=True, encoding="utf-8")
        records = [line.split("\t") for line in output.splitlines()]
        assert len(records) == 20000
        values = []
        for row in records:
            assert row[1].startswith("I32:")
            tag = row[9]
            value = None
            if tag != "N":
                assert tag.startswith("Y64:")
                value = Decimal.from_float(struct.unpack(">d", bytes.fromhex(tag[4:]))[0])
            values.append((int(row[1][4:]), value))
        threshold = min(value for _, value in values if value is not None) + 1
        options = ["--database", path, "--native-bytes", "67108864",
                   "--native-rows", "100000", "--native-steps", "10000000",
                   "--execution-steps", "10000000"]
        predicates = [
            (f"Stamp={threshold}", lambda value: value == threshold),
            (f"Stamp>={threshold}", lambda value: value >= threshold),
            (f"Stamp IN ({threshold},{threshold + 1})", lambda value: value in [threshold, threshold + 1]),
            (f"Stamp BETWEEN {threshold} AND {threshold + 1}", lambda value: threshold <= value <= threshold + 1),
            (f"Stamp=CDec({threshold})", lambda value: value == threshold),
        ]
        for index in [None, "IdAsc", "IdDesc"]:
            configured = options + (["--index", "IndexedRows", index] if index else [])
            for predicate, matches in predicates:
                expected = [[str(identifier)] for identifier, value in values
                            if value is not None and matches(value)]
                expected.sort(key=lambda row: int(row[0]))
                query("run", f"SELECT ID FROM IndexedRows WHERE {predicate} ORDER BY ID", expected, configured)
            count = sum(value is not None and value >= threshold for _, value in values)
            query("run", f"SELECT DCount('*','IndexedRows','Stamp>={threshold}')", [[str(count)]], configured)
        assert path.read_bytes() == before

    print(f"Passed {controls} ordinary Date/numeric predicate controls; native Access unrun")


if __name__ == "__main__":
    main()
