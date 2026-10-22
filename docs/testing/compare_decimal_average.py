"""Original exact-rational Decimal mean controls; no native Access execution."""
import argparse
import csv
from fractions import Fraction
import io
from pathlib import Path
import random
import subprocess


MAXIMUM = (1 << 96) - 1


def literal(coefficient, scale):
    digits = str(abs(coefficient)).rjust(scale + 1, "0")
    if scale:
        digits = digits[:-scale] + "." + digits[-scale:]
    return ("-" if coefficient < 0 else "") + digits


def quantize(value):
    # Fraction.__round__ independently implements ties-to-even. Each candidate
    # scale is derived afresh from the exact mean to avoid double rounding.
    for scale in range(28, -1, -1):
        coefficient = round(value * 10 ** scale)
        if abs(coefficient) <= MAXIMUM:
            return Fraction(coefficient, 10 ** scale)
    raise AssertionError("mean of bounded inputs must fit")


def execute(cli, values):
    inputs = " UNION ALL ".join(
        "SELECT NULL AS Amount" if value is None else
        f"SELECT CDec('{literal(*value)}') AS Amount" for value in values
    )
    sql = f"SELECT Avg(Amount) FROM ({inputs}) AS Inputs"
    output = subprocess.run(
        [str(cli), "query", "run", sql, "--number-format", "invariant", "--format", "tsv"],
        capture_output=True, check=False,
    )
    assert output.returncode == 0, (values, output.stderr)
    rows = list(csv.reader(io.StringIO(output.stdout.decode()), delimiter="\t"))
    assert len(rows) == 2 and len(rows[1]) == 1, rows
    present = [Fraction(n, 10 ** scale) for value in values if value is not None
               for n, scale in [value]]
    if not present:
        assert rows[1][0] == "NULL", rows
    else:
        expected = quantize(sum(present, Fraction()) / len(present))
        assert Fraction(rows[1][0]) == expected, (values, rows, expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    args = parser.parse_args()
    cases = [[None], [(125, 2), (250, 2)], [(MAXIMUM, 0), (3, 28), (-MAXIMUM, 0)]]
    for scale in [0, 1, 14, 28]:
        for sign in [-1, 1]:
            cases.extend([
                [(sign * MAXIMUM, scale)] * 9 + [None],
                [(sign * MAXIMUM, scale), (sign * (MAXIMUM - 1), scale)],
                [(sign * MAXIMUM, scale), (sign * (MAXIMUM - 2), scale)],
            ])
    generator = random.Random(20261022)
    for _ in range(96):
        values = [(generator.choice([-1, 1]) * generator.randrange(MAXIMUM // 2, MAXIMUM),
                   generator.randrange(29)) for _ in range(generator.randrange(2, 10))]
        cases.append(values)
    for values in cases:
        execute(args.cli, values)
    print(f"Passed {len(cases)} independent Fraction/CLI Decimal mean controls; native Access unrun.")


if __name__ == "__main__":
    main()
