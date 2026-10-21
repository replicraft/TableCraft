"""Check Timer against exact rational binary32 rounding, not native Access.

Optionally query AData in original SavedForest-derived fixtures. Database bytes
must remain unchanged. No stored code or native Access execution is involved.
"""

import argparse
import csv
from fractions import Fraction
import hashlib
from io import StringIO
from pathlib import Path
import random
import struct
import subprocess


def represented(bits):
    """The exact value represented by a nonnegative finite binary32 pattern."""
    exponent = (bits >> 23) & 255
    mantissa = bits & ((1 << 23) - 1)
    if exponent == 0:
        return Fraction(mantissa, 1 << 149)

    significand = (1 << 23) | mantissa
    shift = exponent - 127 - 23
    if shift >= 0:
        return Fraction(significand << shift)
    return Fraction(significand, 1 << -shift)


def nearest_bits(value):
    """Binary-search adjacent floats, then choose by exact distance/even parity."""
    low = 0
    high = 0x47A8C000  # Independently specified binary32 86400.
    while low < high:
        middle = (low + high + 1) // 2
        if represented(middle) <= value:
            low = middle
        else:
            high = middle - 1

    if represented(low) == value:
        return low

    upper = low + 1
    below = value - represented(low)
    above = represented(upper) - value
    if below < above:
        return low
    if above < below:
        return upper
    return low if low % 2 == 0 else upper


def query(cli, sql, clock, mode="run", database=None):
    command = [str(cli), "query", mode, sql, "--clock", clock]
    if database is not None:
        command.extend(["--database", str(database)])

    result = subprocess.run(command, capture_output=True, text=True, check=False)
    assert result.returncode == 0, result.stderr
    return list(csv.reader(StringIO(result.stdout), delimiter="\t"))[1:]


def check_timer(text, expected):
    actual = struct.unpack(">I", struct.pack(">f", float(text)))[0]
    assert actual == expected, (text, hex(actual), hex(expected))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()

    # Verify the independent oracle's tie handling without using CLI output.
    assert nearest_bits(Fraction(1) + Fraction(1, 1 << 24)) == 0x3F800000
    assert nearest_bits(Fraction(1) + Fraction(3, 1 << 24)) == 0x3F800002

    rng = random.Random(20261021)
    inputs = [(0, 0), (0, 199), (1, 250_000_000), (47_655, 125_000_000)]
    inputs += [(86_399, 999_999_999)]
    inputs += [(65_536, nanos) for nanos in [3_906_200, 3_906_300, 11_718_700, 11_718_800]]
    inputs += [(rng.randrange(86_400), rng.randrange(1_000_000_000)) for _ in range(128)]
    count = 0
    for mode in ["eval", "run"]:
        for seconds, nanos in inputs:
            hour, remainder = divmod(seconds, 3600)
            minute, second = divmod(remainder, 60)
            clock = f"2026-10-21T{hour:02}:{minute:02}:{second:02}.{nanos:09}"
            expected = nearest_bits(Fraction(seconds) + Fraction(nanos // 100, 10_000_000))
            rows = query(args.cli, "SELECT Timer() AS T,Second(Time()) AS S", clock, mode)
            assert len(rows) == 1 and len(rows[0]) == 2, rows
            check_timer(rows[0][0], expected)
            assert int(rows[0][1]) == second, rows
            count += 2

    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).digest()
        rows = query(
            args.cli,
            "SELECT ID,Timer() AS T FROM AData ORDER BY ID",
            "2026-10-21T13:14:15.125",
            database=database,
        )
        assert [int(row[0]) for row in rows] == [1, 2, 4], rows
        for row in rows:
            check_timer(row[1], nearest_bits(Fraction(381241, 8)))
            count += 1
        assert hashlib.sha256(database.read_bytes()).digest() == before

    print(f"Passed {count} exact-rounding/DATE/native-row controls; native Access unrun.")


if __name__ == "__main__":
    main()
