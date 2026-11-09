"""Original exact binary-rational mean oracle for source-free CLI queries.

Python Fraction.from_float models the input IEEE values, and float(Fraction)
rounds only the final mean. No native Access SQL or third-party fixture is used.
"""
import argparse
import csv
from fractions import Fraction
import io
import itertools
import math
from pathlib import Path
import random
import struct
import subprocess


def execute(cli, values):
    inputs = " UNION ALL ".join(f"SELECT {value!r} AS Amount" for value in values)
    sql = f"SELECT Avg(Amount) FROM ({inputs}) AS Inputs"
    output = subprocess.run([str(cli), "query", "run", sql, "--format", "tsv"], capture_output=True, check=False)
    assert output.returncode == 0, output.stderr
    rows = list(csv.reader(io.StringIO(output.stdout.decode()), delimiter="\t"))
    assert len(rows) == 2 and len(rows[1]) == 1, rows
    expected = float(sum(map(Fraction.from_float, values), Fraction()) / len(values))
    actual = float(rows[1][0])
    assert struct.pack(">d", actual) == struct.pack(">d", expected), (values, actual, expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    args = parser.parse_args()
    maximum = float.fromhex("0x1.fffffffffffffp+1023")
    smallest = float.fromhex("0x0.0000000000001p-1022")
    cases = [
        [maximum, maximum], [-maximum, -maximum],
        [maximum, 1.0, -maximum], [1e308, 1e-300, -1e308],
        [smallest, smallest], [smallest, 0.0], [smallest, 2 * smallest],
        [0.0, -0.0], [1.0, math.nextafter(1.0, math.inf)],
    ]
    cases.extend(map(list, itertools.permutations([1e308, 1.0, -1e308, 1e-300])))
    generator = random.Random(20261022)
    for _ in range(96):
        values = []
        while len(values) < 8:
            value = struct.unpack(">d", struct.pack(">Q", generator.getrandbits(64)))[0]
            if math.isfinite(value):
                values.append(value)
        cases.append(values)
    for values in cases:
        execute(args.cli, values)
    print(f"Passed {len(cases)} independent exact binary Fraction/CLI mean controls; native Access unrun.")


if __name__ == "__main__":
    main()
