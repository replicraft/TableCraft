"""Independent Decimal arithmetic and inert native-field Text coercion controls.

Only TableCraft executes SQL. The original scalar inputs remain byte-for-byte
unchanged; Jackcess supplies field values, never expression evaluation.
"""

import argparse
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute


def numeric(actual, expected):
    assert len(actual) == len(expected), (actual, expected)
    for actual_row, expected_row in zip(actual, expected):
        assert len(actual_row) == len(expected_row), (actual_row, expected_row)
        assert [Decimal(value) for value in actual_row] == expected_row, (actual_row, expected_row)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    count = 0
    for mode, controls in [
        ("eval", ()),
        ("run", ()),
        ("run", ("--declared-arithmetic",)),
    ]:
        for operator in ["+", "-", "*", "/", "\\", "Mod", "^"]:
            for value in [
                "{guid {00112233-4455-6677-8899-aabbccddeeff}}",
                "GUIDFromString('{00112233-4455-6677-8899-aabbccddeeff}')",
            ]:
                for expression in [f"Null {operator} {value}", f"{value} {operator} Null"]:
                    result = subprocess.run(
                        [str(args.cli), "query", mode, "SELECT 1," + expression, *controls],
                        capture_output=True, text=True, check=False,
                    )
                    assert result.returncode == 2 and not result.stdout, (expression, result)
                    assert "expression type mismatch" in result.stderr, (expression, result)
                    count += 1
            sql = f"SELECT Null {operator} 'invalid numeric spelling'"
            assert execute(args.cli, mode, sql, controls=controls) == [["NULL"]]
            count += 1
        sql = "SELECT Null+IIf(True,{guid {00112233-4455-6677-8899-aabbccddeeff}},1/0)"
        result = subprocess.run(
            [str(args.cli), "query", mode, sql, *controls],
            capture_output=True, text=True, check=False,
        )
        assert result.returncode == 2 and not result.stdout
        assert "division by zero" in result.stderr
        count += 1

    values = [Decimal(value) for value in ["-10.5", "-3.5", "-2.5", "-1.25", "-0.5", "0", "0.25", "1.25", "2.5", "3.5", "10.5"]]
    for profile in ["en-us", "de-de"]:
        regional = lambda value: str(value).replace(".", ",") if profile == "de-de" else str(value)
        for value in values:
            text = regional(value)
            divisor = regional(Decimal("2.5"))
            rounded = int(value.to_integral_value(rounding=ROUND_HALF_EVEN))
            quotient = (abs(rounded) // 2) * (-1 if rounded < 0 else 1)
            sql = "SELECT " + ",".join([
                f"'{text}'+3", f"'{text}'-3", f"'{text}'*'2'", f"'{text}'/'2'",
                f"'{text}'\\'{divisor}'", f"'{text}' Mod '{divisor}'",
                f"-'{text}'", f"+'{text}'", f"CDec(1)+'{text}'", f"CCur(1)+'{text}'",
                f"CCur(2)*'{text}'", f"'{text}'^2",
            ])
            expected = [[value + 3, value - 3, value * 2, value / 2,
                         Decimal(quotient), Decimal(rounded - quotient * 2),
                         -value, value, value + 1, value + 1, value * 2, value * value]]
            for mode in ["eval", "run"]:
                numeric(execute(args.cli, mode, sql, controls=("--number-format", profile)), expected)
                count += len(expected[0])
        for mode in ["eval", "run"]:
            near = regional(Decimal("2.5000000000000000000000000000000000001"))
            numeric(execute(args.cli, mode, f"SELECT '{near}'\\1,'-{near}' Mod 2",
                            controls=("--number-format", profile)), [[Decimal(3), Decimal(-1)]])
            count += 2
        for sql in ["SELECT 'bad'+2", "SELECT 'bad'+(1/0)", "SELECT '1'/'0'", "SELECT Null+('1'/0)"]:
            for mode in ["eval", "run"]:
                execute(args.cli, mode, sql, controls=("--number-format", profile), success=False)
                count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        original = database.read_bytes()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        expected = []
        for line in tagged.splitlines():
            identity = int(line.split("\t")[1].removeprefix("I32:"))
            quotient = (abs(identity) // 2) * (-1 if identity < 0 else 1)
            expected.append(list(map(Decimal, [identity, identity + 1, identity * 2, quotient, identity + 1])))
        sql = "SELECT ID,CStr(ID)+1,2*CStr(ID),CStr(ID)\\2,CDec(1)+CStr(ID) FROM ScalarSamples ORDER BY ID"
        for profile in ["en-us", "de-de"]:
            numeric(execute(args.cli, "run", sql, database, controls=("--number-format", profile)), expected)
            count += 5 * len(expected)
        assert database.read_bytes() == original
        print(database.name, len(original), hashlib.sha256(original).hexdigest())
    print(f"Passed {count} independent Decimal/arithmetic/Text/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
