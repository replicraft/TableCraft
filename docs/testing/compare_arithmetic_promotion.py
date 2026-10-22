"""Independent integer range/type, Fraction fixed-point and native getter controls.

The reviewed reference reads stored fields only. Only TableCraft executes SQL;
these controls are not Windows Access result-type or optimizer qualification.
"""

import argparse
from decimal import Decimal
from fractions import Fraction
import hashlib
from pathlib import Path
import struct

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute


def integral(left, right, operation):
    a, rank_a = left
    b, rank_b = right
    value = {"+": a + b, "-": a - b, "*": a * b}[operation]
    rank = max(rank_a, rank_b)
    widths = [(0, 255, 17), (-32768, 32767, 2), (-(2**31), 2**31 - 1, 3), (-(2**63), 2**63 - 1, 20)]
    if rank == 3:
        return (value, 20) if widths[3][0] <= value <= widths[3][1] else None
    for index in range(rank, 3):
        low, high, subtype = widths[index]
        if low <= value <= high:
            return value, subtype
    return float(value), 5


def check(actual, expected):
    assert len(actual) == len(expected), (actual, expected)
    for row, wanted in zip(actual, expected):
        assert len(row) == len(wanted), (row, wanted)
        for value, target in zip(row, wanted):
            assert (value == "NULL" if target is None else Decimal(value) == Decimal(str(target))), (row, wanted)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    constructors = ["CByte", "CInt", "CLng", "CLngLng"]
    values = [(0, 0), (255, 0), (-32768, 1), (32767, 1), (-(2**31), 2), (2**31-1, 2), (-1, 3), (2**63-1, 3)]
    count = 0
    for left in values:
        for right in values:
            for operation in ["+", "-", "*"]:
                expression = f"{constructors[left[1]]}({left[0]}){operation}{constructors[right[1]]}({right[0]})"
                expected = integral(left, right, operation)
                for mode in ["eval", "run"]:
                    if expected is None:
                        execute(args.cli, mode, "SELECT " + expression, success=False)
                        count += 1
                    else:
                        check(execute(args.cli, mode, f"SELECT {expression},VarType({expression})"), [expected])
                        count += 2
    fixed = [(Fraction(1, 10000), Fraction(1, 2)), (Fraction(3, 10000), Fraction(1, 2)), (Fraction(3, 2), Fraction(3, 2))]
    for a, b in fixed:
        text_a = str(Decimal(a.numerator) / Decimal(a.denominator))
        text_b = str(Decimal(b.numerator) / Decimal(b.denominator))
        expected = Decimal(round(a * b * 10000)) / 10000
        for mode in ["eval", "run"]:
            check(execute(args.cli, mode, f"SELECT CCur({text_a})*CCur({text_b}),VarType(CCur({text_a})*CCur({text_b}))"), [[expected, 6]])
            count += 2
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        original = database.read_bytes()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        expected = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            identity = int(fields[1].removeprefix("I32:"))
            octet = None if fields[3] == "N" else integral((int(fields[3][1:]), 0), (1, 0), "+")
            small = None if fields[4] == "N" else integral((int(fields[4][4:]), 1), (1, 1), "+")
            real = None if fields[6] == "N" else struct.unpack(">f", bytes.fromhex(fields[6][1:]))[0] + 1
            expected.append([identity, *(octet if octet else (None, 1)), *(small if small else (None, 1)), real, 1 if real is None else 4])
        sql = "SELECT ID,Octet+CByte(1),VarType(Octet+CByte(1)),Small+CInt(1),VarType(Small+CInt(1)),[Real]+CSng(1),VarType([Real]+CSng(1)) FROM ScalarSamples ORDER BY ID"
        check(execute(args.cli, "run", sql, database), expected)
        count += 7 * len(expected)
        assert database.read_bytes() == original
        print(database.name, len(original), hashlib.sha256(original).hexdigest())
    print(f"Passed {count} independent range/type/Fraction/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
