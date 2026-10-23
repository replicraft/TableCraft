"""Independent numeric Text input/return controls; no native Access SQL runs."""
import argparse
import csv
import datetime as dt
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_numeric_text import floating, spelling


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--native-directory", type=Path)
    args = parser.parse_args()
    controls = 0

    def check(expression, expected, mode, number_format=None, database=None, date_format=None):
        nonlocal controls
        sql = f"SELECT {expression} AS N,VarType({expression}) AS T"
        command = [str(args.cli.resolve()), "query", mode, sql]
        if number_format:
            command += ["--number-format", number_format]
        if date_format:
            command += ["--date-format", date_format]
        if database:
            command[3] += " FROM IndexedRows WHERE ID=1"
            command += [
                "--database", str(database), "--index", "IndexedRows", "IdAsc",
                "--native-bytes", "67108864", "--native-rows", "100000",
                "--native-steps", "10000000", "--execution-steps", "10000000",
            ]
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        if expected is None:
            assert result.returncode == 2 and not result.stdout, (sql, result)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows == [["N", "T"], [spelling(expected), "8"]], (sql, rows, expected)
        controls += 1

    inputs = ["0", "-0", "2.5", "3.5", "-2.5", "-32767.5", "-32768.5",
              "459", "-1", "2147483647", "-2147483648"]
    for mode in ["eval", "run"]:
        for number_format, separator in [("en-us", "."), ("de-de", ",")]:
            for text in inputs:
                rounded = int(Decimal(text).to_integral_value(rounding=ROUND_HALF_EVEN))
                bits = rounded if rounded >= 0 else rounded % (2 ** (16 if rounded > -32768 else 32))
                localized = text.replace(".", separator)
                for function, expected in [
                    ("Hex", format(bits, "X")), ("Oct", format(bits, "o")),
                    ("Str", floating(float(text), 15)),
                ]:
                    check(f"{function}('{localized}')", expected, mode, number_format)
                    check(f"{function}('{localized}')", None, mode)
            for expression in ["Hex('2147483648')", "Oct('-2147483649')",
                               "Str('bad')", "Str('1e999')", "Hex('bad',1/0)"]:
                check(expression, None, mode, number_format)
    for mode in ["eval", "run"]:
        for code in [0, 1, 2001, 65535]:
            check(f"Str(CVErr({code}))", f"Error {code}", mode)
        for expression, expected in [
            ("Str(Nz(NULL,0))", " 0"),
            ("Str(CVar(CVErr(1)))", "Error 1"),
            ("Str(IIf(True,CVErr(1),0))", "Error 1"),
            ("Str(CDate(2))", "1900-01-01"),
        ]:
            check(expression, expected, mode, date_format="iso")
        for expression in ["Str(CDate(2))", "Str(IIf(True,CVErr(1),1/0))", "Str(CVErr(65536))"]:
            check(expression, None, mode)
    for mode in ["eval", "run"]:
        for text in ["2.5", "3.5", "-1.5", "-32768.5", "-657434.75", "2958465.75"]:
            rounded = int(Decimal(text).to_integral_value(rounding=ROUND_HALF_EVEN))
            bits = rounded if rounded >= 0 else rounded % (2 ** (16 if rounded > -32768 else 32))
            check(f"Hex(CDate({text}))", format(bits, "X"), mode)
            check(f"Oct(CDate({text}))", format(bits, "o"), mode)
        for expression in ["Hex(CVErr(1))", "Oct(CVErr(1))", "Hex(IIf(True,CVErr(1),1/0))"]:
            check(expression, None, mode)
        check("Hex(CLng(CVErr(1)))", "1", mode)
    if args.native_directory:
        for suffix, digest in INPUT_HASHES.items():
            database = args.native_directory / ("index-" + suffix)
            digest = registered_input_hash(database, digest)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
            for number_format in ["en-us", "de-de"]:
                for function, expected in [("Hex", "1"), ("Oct", "1"), ("Str", " 1")]:
                    check(f"{function}(CStr(ID))", expected, "run", number_format, database)
            stamp_days = Decimal((dt.date(2024, 1, 2) - dt.date(1899, 12, 30)).days) + Decimal("0.5")
            stamp_long = int(stamp_days.to_integral_value(rounding=ROUND_HALF_EVEN))
            for function, expected in [("Hex", format(stamp_long, "X")), ("Oct", format(stamp_long, "o"))]:
                check(f"{function}(Stamp)", expected, "run", database=database)
            check("Str(CVErr(ID))", "Error 1", "run", database=database)
            check("Str(CDate(ID))", "1899-12-31", "run", database=database, date_format="iso")
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
    print(f"Passed {controls} numeric Text input controls; native Access remains unqualified.")


if __name__ == "__main__":
    main()
