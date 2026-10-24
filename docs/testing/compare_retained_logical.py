"""Independent bitwise answers for retained numeric Strings and eager failures.

Only TableCraft executes SQL. Decimal nearest-even rounding and Python per-bit
truth tables supply answers independently of the Rust implementation. No native
database, VBA runtime or Microsoft Access execution is involved.
"""

import argparse
import csv
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess


OPERATORS = [
    ("And", [0, 0, 0, 1]),
    ("Or", [0, 1, 1, 1]),
    ("Xor", [0, 1, 1, 0]),
    ("Eqv", [1, 0, 0, 1]),
    ("Imp", [1, 1, 0, 1]),
]


def signed(value, width):
    return value - 2 ** width if value >= 2 ** (width - 1) else value


def bit_result(left, right, width, truth):
    result = 0
    for position in range(width):
        a = (left >> position) & 1
        b = (right >> position) & 1
        result += truth[a * 2 + b] * 2 ** position
    return signed(result, width)


def slices(text):
    size = len(text.encode("utf-16-le"))
    return [
        f"LeftB('{text}x',{size})",
        f"RightB('x{text}',{size})",
        f"MidB('x{text}y',3,{size})",
        f"MidB(MidB('x{text}y',2,{size + 1}),2,{size})",
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    before = hashlib.sha256(args.cli.read_bytes()).hexdigest()
    queries = 0
    values_checked = 0
    refusals = 0

    def query(mode, sql, profile="en-US", declared=False, failure=None):
        nonlocal queries, values_checked, refusals
        command = [str(args.cli), "query", mode, sql]
        if profile is not None:
            command += ["--number-format", profile]
        if declared:
            command.append("--declared-arithmetic")
        result = subprocess.run(
            command, capture_output=True, text=True, encoding="utf-8", check=False,
        )
        queries += 1
        if failure is not None:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert failure in result.stderr.lower(), (sql, result.stderr)
            refusals += 1
            return None
        assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
        rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))[1:]
        values_checked += sum(len(row) for row in rows)
        return rows

    numbers = [
        "-2147483648", "-3.5", "-2.5", "-1.5", "-0.5", "-0", "0.25",
        "0.5", "1.5", "2.5", "3.5", "2147483647",
    ]
    for profile in ["en-US", "de-DE"]:
        for text in numbers:
            number = Decimal(text)
            rounded = int(number.to_integral_value(rounding=ROUND_HALF_EVEN))
            regional = text.replace(".", ",") if profile == "de-DE" else text
            for operand in slices(regional):
                expressions = [f"{operand} {op} CLng(3)" for op, _ in OPERATORS]
                expressions.append(f"Not {operand}")
                answers = [bit_result(rounded, 3, 32, table) for _, table in OPERATORS]
                answers.append(signed((2 ** 32 - 1) ^ (rounded % 2 ** 32), 32))
                for expression, answer in zip(expressions, answers, strict=True):
                    sql = f"SELECT {expression},VarType({expression})"
                    expected = [[str(answer), "3"]]
                    for mode in ["eval", "run"]:
                        assert query(mode, sql, profile) == expected, sql

    for number in [-9223372036854775808, -9007199254740993, 9007199254740993, 9223372036854775807]:
        for operand in slices(str(number)):
            for reverse in [False, True]:
                left = "CLngLng(9007199254740993)" if reverse else operand
                right = operand if reverse else "CLngLng(9007199254740993)"
                a, b = (9007199254740993, number) if reverse else (number, 9007199254740993)
                expressions = [f"{left} {op} {right}" for op, _ in OPERATORS]
                answers = [bit_result(a, b, 64, table) for _, table in OPERATORS]
                for expression, answer in zip(expressions, answers, strict=True):
                    sql = f"SELECT {expression},VarType({expression})"
                    expected = [[str(answer), "20"]]
                    for mode in ["eval", "run"]:
                        assert query(mode, sql) == expected, sql

    for number in [0, -1, 2]:
        for operand in slices(str(number)):
            for operator, _ in OPERATORS:
                for reverse in [False, True]:
                    if operator == "And":
                        result = 0 if number == 0 else None
                    elif operator == "Or":
                        result = number
                    elif operator == "Imp" and reverse:
                        result = number if number != 0 else None
                    elif operator == "Imp":
                        result = -number - 1 if number != -1 else None
                    else:
                        result = None
                    left, right = ("NULL", operand) if reverse else (operand, "NULL")
                    expression = f"{left} {operator} {right}"
                    expected = [[str(result), "3"]] if result is not None else [["NULL", "1"]]
                    for mode in ["eval", "run"]:
                        assert query(mode, f"SELECT {expression},VarType({expression})") == expected

    for operator, _ in OPERATORS:
        for raw in ["ChrW(55296)", "ChrB(65)", "LeftB('😀',2)"]:
            for mode in ["eval", "run"]:
                query(mode, f"SELECT 1,{raw} {operator} (1/0)", failure="division by zero")
                query(mode, f"SELECT 1,{raw} {operator} CVErr(11)", failure="type mismatch")
                for peer in ["NULL", "CInt(0)"]:
                    for left, right in [(raw, peer), (peer, raw)]:
                        query(mode, f"SELECT 1,{left} {operator} {right}", failure="utf-16")
                query(mode, f"SELECT 1,CDbl(2147483648) {operator} {raw}", failure="overflow")
            for sql in [
                f"SELECT D.S {operator} (1/0) FROM (SELECT {raw} AS S) AS D",
                f"SELECT First({raw} {operator} (1/0))",
                f"SELECT (SELECT D.S {operator} (1/0)) FROM (SELECT {raw} AS S) AS D",
                f"SELECT Count(*) FROM (SELECT {raw} AS S) AS D GROUP BY D.S {operator} (1/0)",
                f"SELECT 1 WHERE {raw} {operator} (1/0)",
                f"SELECT Count(*) HAVING {raw} {operator} (1/0)",
            ]:
                query("run", sql, failure="division by zero")

    for mode in ["eval", "run"]:
        for raw in ["ChrW(55296)", "ChrB(65)", "LeftB('😀',2)"]:
            query(mode, f"SELECT 1,Not {raw}", failure="utf-16")
        query(
            mode, "SELECT 1,LeftB('1',2) And ChrW(55296)",
            profile=None, failure="number-format",
        )

    for operand in slices("-2147483648"):
        for wrapper, variant in [("{}", True), ("CVar({})", True), ("CStr({})", False)]:
            value = wrapper.format(operand)
            for sql in [
                f"SELECT (Not {value})+CLng(1)",
                f"SELECT D.N+CLng(1) FROM (SELECT Not {value} AS N) AS D",
                f"SELECT (SELECT D.N+CLng(1)) FROM (SELECT Not {value} AS N) AS D",
            ]:
                if variant:
                    assert query("run", sql, declared=True) == [["2147483648"]]
                else:
                    query("run", sql, declared=True, failure="overflow")

    assert hashlib.sha256(args.cli.read_bytes()).hexdigest() == before
    print(json.dumps({
        "queries": queries,
        "values": values_checked,
        "refusals": refusals,
        "cli_sha256": before,
        "model": "Decimal nearest-even and per-bit signed 32/64-bit truth tables",
        "native_access": "unrun",
    }, sort_keys=True))


if __name__ == "__main__":
    main()
