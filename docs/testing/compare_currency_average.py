"""Original exact-rational Currency mean controls; no native Access execution.

Source-free UNION inputs exercise the actual CLI query evaluator. Fraction supplies
independent exact means for integer and quarter-unit inputs with power-of-two counts.
"""
import argparse
import csv
from fractions import Fraction
import io
from pathlib import Path
import random
import subprocess


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
    args = parser.parse_args()
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
    for values in cases:
        execute(args.cli, values)
    print(f"Passed {len(cases)} independent Fraction/CLI Currency mean controls; native Access unrun.")


if __name__ == "__main__":
    main()
