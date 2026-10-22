"""Independent Fraction, binary32 and raw native-field operator controls.

The reference supplies inert raw values only. TableCraft alone executes SQL;
materialized result models do not establish Windows Access compatibility.
"""

import argparse
from fractions import Fraction
import hashlib
from pathlib import Path
import struct

from compare_arithmetic_promotion import check
from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    count = 0
    cases = [
        ("-CByte(255)", -255, 2), ("-CInt(-32768)", 32768, 3),
        ("-CLng(-2147483648)", 2147483648, 5), ("-CSng(1.25)", -1.25, 4),
        ("CSng(3)/CInt(2)", float(Fraction(3, 2)), 4),
        ("CSng(3)/CLng(2)", float(Fraction(3, 2)), 5),
        ("CCur(3)/CCur(2)", float(Fraction(3, 2)), 5),
        ("CDate(-1.25)/CInt(2)", float(Fraction(-5, 8)), 5),
        ("CCur(3)^CInt(2)", 9, 5), ("CDate(1.25)^CInt(2)", float(Fraction(25, 16)), 5),
        ("CByte(9)\\CByte(2)", 4, 17), ("CInt(-9)\\CInt(2)", -4, 2),
        ("CInt(-32768)\\CInt(-1)", 32768, 3),
        ("CLng(-2147483648)\\CLng(-1)", 2147483648, 5),
        ("CInt(9)\\CDbl(2.5)", 4, 2), ("CDbl(9.5)\\CInt(2)", 5, 3),
        ("CDbl(-9.5) Mod CInt(3)", -1, 3),
        ("CLngLng(-9223372036854775808) Mod CInt(-1)", 0, 20),
        ("CDec(3)/CDec(2)", 1.5, 14),
    ]
    for mode in ["eval", "run"]:
        for expression, value, subtype in cases:
            check(execute(args.cli, mode, f"SELECT {expression},VarType({expression})"), [[value, subtype]])
            count += 2
        for left in range(-17, 18):
            for right in [-3, -2, 2, 3]:
                a, b = round(Fraction(left, 2)), round(Fraction(right, 2))
                quotient = (abs(a) // abs(b)) * (-1 if (a < 0) != (b < 0) else 1)
                remainder = a - quotient * b
                expression = f"CSng({left / 2})"
                divisor = f"CSng({right / 2})"
                check(execute(args.cli, mode, f"SELECT {expression}\\{divisor},{expression} Mod {divisor},VarType({expression}\\{divisor})"), [[quotient, remainder, 3]])
                count += 3
        for profile in ["en-us", "de-de"]:
            separator = "," if profile == "de-de" else "."
            near = "2" + separator + "5000000000000000000000000000000000001"
            check(execute(args.cli, mode, f"SELECT '{near}'\\CInt(1),VarType('{near}'\\CInt(1))", controls=("--number-format", profile)), [[3, 3]])
            count += 2
            for sql in ["SELECT CInt(9)\\'32767" + separator + "5'", "SELECT '2147483647" + separator + "5' Mod '2'", "SELECT CCur(1)/CCur(0)", "SELECT Null+(CCur(1)/CCur(0))", "SELECT -CLngLng(-9223372036854775808)"]:
                execute(args.cli, mode, sql, controls=("--number-format", profile), success=False)
                count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        original = database.read_bytes()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "--date-bits", "ScalarSamples")
        expected = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            identity = int(fields[1].removeprefix("I32:"))
            octet = None if fields[3] == "N" else int(fields[3][1:])
            small = None if fields[4] == "N" else int(fields[4][4:])
            real = None if fields[6] == "N" else struct.unpack(">f", bytes.fromhex(fields[6][1:]))[0]
            stamp = None if fields[-1] == "N" else struct.unpack(">d", bytes.fromhex(fields[-1][4:]))[0]
            expected.append([identity, None if octet is None else -octet, 1 if octet is None else 2,
                             None if small is None else -small, 1 if small is None else (3 if small == -32768 else 2),
                             None if real is None else real / 2, 1 if real is None else 4,
                             None if stamp is None else stamp / 2])
        sql = "SELECT ID,-Octet,VarType(-Octet),-Small,VarType(-Small),[Real]/CInt(2),VarType([Real]/CInt(2)),Stamp/2 FROM ScalarSamples ORDER BY ID"
        check(execute(args.cli, "run", sql, database), expected)
        count += 8 * len(expected)
        assert database.read_bytes() == original
        print(database.name, len(original), hashlib.sha256(original).hexdigest())
    print(f"Passed {count} independent Fraction/rounding/type/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
