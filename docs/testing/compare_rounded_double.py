"""Independent integer-significand/IEEE models for Double-selected arithmetic.

Only TableCraft executes SQL; native Access equivalence remains unqualified.
"""
import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES
from compare_union_types import JAR_SHA256
from compare_arithmetic_promotion import check


def rounded_integer(value):
    magnitude = abs(value)
    discarded = max(0, magnitude.bit_length() - 53)
    unit = 1 << discarded
    quotient, remainder = divmod(magnitude, unit)
    if 2 * remainder > unit or (2 * remainder == unit and quotient % 2):
        quotient += 1
    rounded = float(quotient) * unit
    return -rounded if value < 0 else rounded


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def query(mode, sql, expected, options=(), diagnostic=None):
        nonlocal controls
        result = subprocess.run([str(args.cli), "query", mode, sql, *map(str, options)],
                                text=True, encoding="utf-8", capture_output=True)
        if diagnostic:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert diagnostic in result.stderr, (sql, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            check(rows[1:], expected)
        controls += 1

    values = [-(2**63), -(2**53+3), -(2**53+1), -1, 0, 1, 2**53-1,
              2**53+1, 2**53+3, 2**63-1]
    for mode in ["eval", "run"]:
        for integer in values:
            operand = f"CLngLng({integer})"
            rounded = rounded_integer(integer)
            for expression, expected in [
                (operand + "+CDbl(1)", rounded + 1.0),
                ("CSng(1)+" + operand, 1.0 + rounded),
                (operand + "-CDbl(1)", rounded - 1.0),
                ("CDbl(1)-" + operand, 1.0 - rounded),
                (operand + "*CDbl(0.5)", rounded * 0.5),
                (operand + "/CLngLng(1)", rounded),
                (operand + "^CByte(1)", rounded),
            ]:
                query(mode, f"SELECT {expression},VarType({expression})", [[expected, 5]])
            query(mode, f"SELECT {operand}+CByte(0)", [[integer]])
        query(mode, "SELECT Null+(9007199254740993/0)", None, diagnostic="division by zero")
        query(mode, "SELECT 9223372036854775807+CByte(1)", None, diagnostic="overflow")

    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        raw = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows"], text=True, encoding="utf-8")
        ids = [int(line.split("\t")[1][4:]) for line in raw.splitlines()]
        assert len(ids) == 20000
        options = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
                   "--native-steps", "10000000", "--execution-steps", "10000000"]
        expression = "(ID+9007199254740990)+CDbl(1)"
        sql = f"SELECT ID,{expression},VarType({expression}) FROM IndexedRows WHERE ID<=4 ORDER BY ID"
        expected = [[value, rounded_integer(value+9007199254740990)+1.0, 5] for value in sorted(ids) if value <= 4]
        for index in [None, "IdAsc", "IdDesc"]:
            query("run", sql, expected, [*options, *(["--index", "IndexedRows", index] if index else [])])
        assert path.read_bytes() == before
    print(f"Passed {controls} independent rounded-Double controls; native Access unrun.")


if __name__ == "__main__":
    main()
