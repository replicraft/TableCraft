"""Independent Fraction-model controls for fixed-point statistical offsets.

No native Access execution or database fixture is used. Mixed floating groups
retain the existing binary64 conversion model; all-fixed groups retain exact
input differences before the final statistical floating calculations.
"""

import argparse
import csv
from fractions import Fraction
import hashlib
from io import StringIO
import itertools
import json
import math
from pathlib import Path
import subprocess


def spelling(coefficient, scale):
    sign = "-" if coefficient < 0 else ""
    digits = str(abs(coefficient)).rjust(scale + 1, "0")
    if scale:
        digits = digits[:-scale] + "." + digits[-scale:]
    return sign + digits


def moments(values):
    values = [value for value in values if value is not None]
    count = len(values)
    if not count:
        return [None] * 4
    mean = sum(values, Fraction()) / count
    squares = sum(((value - mean) ** 2 for value in values), Fraction())
    sample = squares / (count - 1) if count >= 2 else None
    population = squares / count
    return [
        float(sample) if sample is not None else None,
        float(population) if count >= 2 else None,
        math.sqrt(sample) if sample is not None else None,
        math.sqrt(population),
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    binary = args.cli.resolve()
    before = hashlib.sha256(binary.read_bytes()).hexdigest()
    controls = 0
    queries = 0
    query_digest = hashlib.sha256()

    def check(rows, values, expression="X"):
        nonlocal controls, queries
        source = " UNION ALL ".join(
            f"SELECT {row} AS X,{index} AS ID" for index, row in enumerate(rows)
        )
        sql = (
            f"SELECT Var({expression}) AS V,VarP({expression}) AS VP,"
            f"StDev({expression}) AS S,StDevP({expression}) AS SP "
            f"FROM ({source}) AS D"
        )
        query_digest.update(sql.encode())
        query_digest.update(b"\n")
        result = subprocess.run(
            [str(binary), "query", "run", sql, "--format", "tsv", "--number-format", "en-us"],
            capture_output=True, text=True, check=False,
        )
        assert result.returncode == 0 and not result.stderr, (sql, result)
        output = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        assert len(output) == 2 and output[0] == ["V", "VP", "S", "SP"], (sql, output)
        for cell, expected in zip(output[1], moments(values), strict=True):
            if expected is None:
                assert cell == "NULL", (sql, cell)
            else:
                actual = float(cell)
                assert math.isfinite(actual), (sql, cell)
                assert math.isclose(actual, expected, rel_tol=2e-13, abs_tol=0.0), (
                    sql, actual, expected,
                )
            controls += 1
        queries += 1

    maximum = (1 << 96) - 1
    groups = [
        ("CLngLng", 0, [1 << 53, (1 << 53) + 1, (1 << 53) + 2]),
        ("CLngLng", 0, [(1 << 63) - 3, (1 << 63) - 2, (1 << 63) - 1]),
        ("CLngLng", 0, [-(1 << 63), -(1 << 63) + 1, -(1 << 63) + 2]),
        ("CCur", 4, [(1 << 63) - 3, (1 << 63) - 2, (1 << 63) - 1]),
        ("CCur", 4, [-(1 << 63), -(1 << 63) + 1, -(1 << 63) + 2]),
        ("CDec", 28, [maximum - 2, maximum - 1, maximum]),
        ("CDec", 28, [-maximum, -maximum + 1, -maximum + 2]),
    ]
    for cast, scale, coefficients in groups:
        for permutation in itertools.permutations(coefficients):
            rows = [f"{cast}('{spelling(value, scale)}')" for value in permutation]
            values = [Fraction(value, 10 ** scale) for value in permutation]
            check(rows, values)
            check(["NULL", *rows, "NULL"], [None, *values, None])

    for pairs in [
        [(1, 0), (10 ** 28 + 1, 28), (10 ** 28 + 2, 28)],
        [(maximum, 0), (1, 28), (-maximum, 0)],
    ]:
        for permutation in itertools.permutations(pairs):
            check(
                [f"CDec('{spelling(value, scale)}')" for value, scale in permutation],
                [Fraction(value, 10 ** scale) for value, scale in permutation],
            )

    large = [1 << 53, (1 << 53) + 1, (1 << 53) + 2]
    rows = [f"CLngLng('{value}')" for value in large]
    # Explicit CDbl and both mixed orders retain binary64-rounded input semantics.
    rounded = [Fraction.from_float(float(value)) for value in large]
    for expression in ["CDbl(X)", "IIf(ID=1,CDbl(X),X)", "IIf(ID=1,X,CDbl(X))"]:
        check(rows, rounded, expression)
    check(["CDec('1')"], [Fraction(1)])
    check(["NULL", "NULL"], [None, None])

    after = hashlib.sha256(binary.read_bytes()).hexdigest()
    assert before == after, (before, after)
    summary = f"Passed {controls} Fraction-model statistical controls across {queries} SQL queries; binary SHA256 {before}"
    if args.report:
        report = {
            "date": "2026-10-24",
            "scope": "Exact fixed-point offset statistics and unchanged mixed floating conversion; source-free SQL",
            "native_access_qualification": "unrun",
            "binary_sha256_before": before,
            "binary_sha256_after": after,
            "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "queries": queries,
            "controls": controls,
            "generated_sql_sha256": query_digest.hexdigest(),
            "reference": "Python Fraction exact centered moments, then binary64 conversion/sqrt; no production imports",
            "stdout": summary + "\n",
        }
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(summary)


if __name__ == "__main__":
    main()
