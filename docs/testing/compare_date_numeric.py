"""Original numeric DATE conversion and inert native-field controls.

Calendar/Decimal/IEEE models provide expected results independently of the Rust
conversion kernels. Quarter-day inputs avoid unspecified Decimal normalization
of nonexact binary fractions. This does not execute native Access SQL.
"""

import argparse
import datetime as dt
from decimal import Decimal, ROUND_HALF_EVEN, localcontext
import hashlib
import math
from pathlib import Path
import random
import struct

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute
from compare_format_dates import literal
from compare_format_serials import serial
from compare_large_indexes import INPUT_HASHES, registered_input_hash


def single(value):
    return struct.unpack("<f", struct.pack("<f", float(value)))[0]


def expected(value):
    rounded = int(value.to_integral_value(rounding=ROUND_HALF_EVEN))
    result = [
        ("CLng", rounded), ("CLngLng", rounded), ("CBool", bool(value)),
        ("CSng", single(value)), ("CDbl", float(value)),
        ("CCur", value.quantize(Decimal("0.0001"), rounding=ROUND_HALF_EVEN)),
        ("CDec", value),
    ]
    if -32768 <= rounded <= 32767:
        result.append(("CInt", rounded))
    if 0 <= rounded <= 255:
        result.append(("CByte", rounded))
    return result


def check_cell(actual, expected_value, name):
    if isinstance(expected_value, bool):
        assert actual == ("TRUE" if expected_value else "FALSE"), (actual, expected_value)
    elif isinstance(expected_value, float):
        # CLI Single text is shortest binary32-roundtrip spelling, not a
        # binary64 spelling of the same value. Decode at its declared width.
        value = single(Decimal(actual)) if name == "CSng" else float(actual)
        assert value == expected_value, (actual, expected_value)
    else:
        assert Decimal(actual) == expected_value, (actual, expected_value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    parser.add_argument("--native-directory", type=Path)
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261021)
    values = [dt.datetime(100, 1, 1), dt.datetime(1899, 12, 29, 6),
              dt.datetime(1899, 12, 30), dt.datetime(1900, 1, 1, 12),
              dt.datetime(2025, 1, 1, 12), dt.datetime(9999, 12, 31, 18)]
    values += [dt.datetime(100, 1, 1) + dt.timedelta(
        days=rng.randrange(3_000_000), hours=rng.randrange(4) * 6
    ) for _ in range(48)]
    count = 0
    for mode in ["eval", "run"]:
        for value in values:
            probes = expected(serial(value))
            sql = "SELECT " + ",".join(f"{name}({literal(value)})" for name, _ in probes)
            rows = execute(args.cli, mode, sql)
            assert len(rows) == 1 and len(rows[0]) == len(probes)
            for cell, (name, expected_value) in zip(rows[0], probes):
                check_cell(cell, expected_value, name)
                count += 1
        for expression in [
            "CByte(#1899-12-29#)", "CInt(#2025-01-01#)", "CDbl(Null)",
            "CDbl(#2025-01-01#),1/0", "CStr(#2025-01-01#)",
        ]:
            execute(args.cli, mode, "SELECT " + expression, success=False)
            count += 1
        row = execute(args.cli, mode,
                      "SELECT CSng(#1899-12-30 00:00:01#),CDbl(#1899-12-30 00:00:01#)")[0]
        check_cell(row[0], single(Decimal(1) / Decimal(86400)), "CSng")
        check_cell(row[1], float(Decimal(1) / Decimal(86400)), "CDbl")
        count += 2
    math_values = [-657434.75, -657434.0, -1.75, -0.25, 0.0, 0.25, 2.75, 2958465.75]
    math_values += [rng.randrange(-2629739, 11833864) / 4 for _ in range(64)]
    for mode in ["eval", "run"]:
        for number in math_values:
            probes = [
                ("Abs", abs(number), 5), ("Sgn", (number > 0) - (number < 0), 2),
                ("Fix", math.trunc(number), 7),
            ]
            if math.floor(number) >= -657434:
                probes.append(("Int", math.floor(number), 7))
            expressions = []
            for name, _, _ in probes:
                expression = f"{name}(CDate({number!r}))"
                expressions += [f"CDbl({expression})" if name in ["Int", "Fix"] else expression,
                                f"VarType({expression})"]
            rows = execute(args.cli, mode, "SELECT " + ",".join(expressions))
            assert len(rows) == 1 and len(rows[0]) == 2 * len(probes), rows
            for index, (_, expected_value, tag) in enumerate(probes):
                assert float(rows[0][2 * index]) == expected_value, (number, rows, expected_value)
                assert rows[0][2 * index + 1] == str(tag), (number, rows, tag)
                count += 1
        for expression in ["Int(CDate(-657434.75))", "Round(CDate(2.5))", "Abs(CDate(1/0))"]:
            execute(args.cli, mode, "SELECT 1," + expression, success=False)
            count += 1

    for mode in ["eval", "run"]:
        for lexical in ["-1", "-0.75", "-0.5", "-0.0", "0.5", "1", "1.5", "2.5", "28.5", "28.75", "29"]:
            places = int(Decimal(lexical).to_integral_value(rounding=ROUND_HALF_EVEN))
            for source in ["2.25", "CCur(2.25)", "CDec(2.25)", "NULL"]:
                expression = f"Round({source},CDate({lexical}))"
                if not 0 <= places <= 28:
                    execute(args.cli, mode, "SELECT 1," + expression, success=False)
                else:
                    rows = execute(args.cli, mode, "SELECT " + expression)
                    actual = rows[0][0]
                    if source == "NULL":
                        assert actual == "NULL", (expression, rows)
                    else:
                        with localcontext() as context:
                            context.prec = 50
                            expected_value = Decimal("2.25").quantize(
                                Decimal(1).scaleb(-places), rounding=ROUND_HALF_EVEN
                            )
                        assert Decimal(actual) == expected_value, (expression, rows, expected_value)
                count += 1

    if args.native_directory:
        number = (dt.datetime(2024, 1, 2, 12) - dt.datetime(1899, 12, 30)).total_seconds() / 86400
        expected_row = [number, 5, math.floor(number), 7, math.trunc(number), 7, 1, 2]
        for suffix, digest in INPUT_HASHES.items():
            database = args.native_directory / ("index-" + suffix)
            digest = registered_input_hash(database, digest)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
            sql = "SELECT Abs(Stamp),VarType(Abs(Stamp)),CDbl(Int(Stamp)),VarType(Int(Stamp)),CDbl(Fix(Stamp)),VarType(Fix(Stamp)),Sgn(Stamp),VarType(Sgn(Stamp)) FROM IndexedRows WHERE ID=1"
            rows = execute(args.cli, "run", sql, database, controls=[
                "--index", "IndexedRows", "IdAsc", "--native-bytes", "67108864",
                "--native-rows", "100000", "--native-steps", "10000000",
                "--execution-steps", "10000000",
            ])
            assert len(rows) == 1 and list(map(float, rows[0])) == expected_row, rows
            count += 4
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        reference = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            if fields[-1] != "N":
                value = dt.datetime.fromisoformat(fields[-1].removeprefix("Y"))
                reference.append((fields[1].removeprefix("I32:"), serial(value)))
        names = ["CLng", "CLngLng", "CBool", "CSng", "CDbl", "CCur", "CDec"]
        sql = "SELECT ID," + ",".join(f"{name}(Stamp)" for name in names)
        sql += " FROM ScalarSamples WHERE Stamp IS NOT NULL ORDER BY ID"
        rows = execute(args.cli, "run", sql, database)
        assert len(rows) == len(reference)
        for row, (identity, value) in zip(rows, reference):
            assert row[0] == identity
            probes = dict(expected(value))
            assert len(row) == len(names) + 1
            for name, cell in zip(names, row[1:]):
                check_cell(cell, probes[name], name)
                count += 1
        execute(args.cli, "run", "SELECT CDbl(Stamp) FROM ScalarSamples", database, success=False)
        count += 1
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent DATE/numeric/overflow/native-field controls; native Access unrun.")


if __name__ == "__main__":
    with localcontext() as context:
        context.prec = 1000
        main()
