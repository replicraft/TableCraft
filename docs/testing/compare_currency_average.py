"""Original exact-rational Currency mean controls; no native Access execution.

Source-free UNION inputs exercise the actual CLI query evaluator. Fraction supplies
independent exact means; optional inert native getters add a double-rounding regression.
"""
import argparse
import csv
from fractions import Fraction
import hashlib
import io
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES
from compare_union_types import JAR_SHA256


def execute(cli, values):
    inputs = " UNION ALL ".join(
        "SELECT NULL AS Amount" if value is None else
        f"SELECT CCur({value.numerator}/{value.denominator}) AS Amount"
        for value in values
    )
    # Closed scalar integral literals avoid floating conversions at the Currency
    # range edge. Integer division by one still has a floating result, so omit it.
    for value in values:
        if value is not None and value.denominator == 1:
            inputs = inputs.replace(f"CCur({value.numerator}/1)", f"CCur({value.numerator})")
    sql = f"SELECT Avg(Amount) FROM ({inputs}) AS Inputs"
    output = subprocess.run([str(cli), "query", "run", sql, "--format", "tsv"], capture_output=True, check=False)
    assert output.returncode == 0, output.stderr
    rows = list(csv.reader(io.StringIO(output.stdout.decode()), delimiter="\t"))
    present = [value for value in values if value is not None]
    expected = float(sum(present, Fraction()) / len(present)) if present else None
    assert len(rows) == 2 and len(rows[1]) == 1, rows
    if expected is None:
        assert rows[1][0] == "NULL", rows
    else:
        assert float(rows[1][0]) == expected, (values, rows, expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--native-directory", type=Path)
    parser.add_argument("--jar", type=Path)
    args = parser.parse_args()
    if (args.native_directory is None) != (args.jar is None):
        parser.error("--native-directory and --jar must be supplied together")
    cases = [
        [None],
        [Fraction(5, 4), Fraction(5, 2)],
        [Fraction(922337203685477), Fraction(922337203685477)],
        [Fraction(-922337203685477), Fraction(-922337203685477)],
        [Fraction(922337203685477), Fraction(-922337203685477)],
    ]
    generator = random.Random(20261022)
    for _ in range(64):
        cases.append([Fraction(generator.randint(-4000000, 4000000), 4) for _ in range(4)])
    for _ in range(32):
        cases.append([Fraction(generator.randint(-4000000, 4000000), 4)
                      for _ in range(generator.randrange(3, 32, 2))])
    for values in cases:
        execute(args.cli, values)
    controls = len(cases)
    if args.jar is not None:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
        for suffix, digest in INPUT_HASHES.items():
            path = args.native_directory / ("index-" + suffix)
            before = path.read_bytes()
            assert hashlib.sha256(before).hexdigest() == digest
            output = subprocess.check_output([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows",
            ], text=True, encoding="utf-8")
            records = [line.split("\t") for line in output.splitlines()]
            assert len(records) == 20000
            selected = [row for row in records if int(row[1][4:]) <= 16913]
            assert len(selected) == 16913
            for sign in [1, -1]:
                expression = f"IIf(ID=2,[Money]*{sign * 5},CCur(0))"
                total = sum((Fraction(int(row[6][1:]) * sign * 5, 10000)
                             for row in selected if row[1] == "I32:2"), Fraction())
                expected = float(total / len(selected))
                assert abs(total) == Fraction(5, 10000)
                for index in [None, "IdAsc", "IdDesc"]:
                    sql = (f"SELECT Avg({expression}),DAvg('{expression}','IndexedRows','ID<=16913') "
                           "FROM IndexedRows WHERE ID<=16913")
                    command = [str(args.cli), "query", "run", sql, "--format", "tsv",
                               "--database", str(path), "--native-bytes", "67108864",
                               "--native-rows", "100000", "--native-steps", "10000000",
                               "--execution-steps", "10000000"]
                    if index is not None:
                        command += ["--index", "IndexedRows", index]
                    result = subprocess.run(command, capture_output=True, text=True, check=False)
                    assert result.returncode == 0 and not result.stderr, (sql, result)
                    rows = list(csv.reader(io.StringIO(result.stdout), delimiter="\t", strict=True))
                    assert len(rows) == 2 and len(rows[1]) == 2, rows
                    assert [float(value) for value in rows[1]] == [expected, expected], (sql, rows, expected)
                    controls += 1
            assert path.read_bytes() == before
    print(f"Passed {controls} independent Fraction/CLI/native Currency mean controls; native Access unrun.")


if __name__ == "__main__":
    main()
