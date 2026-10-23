"""Independent numeric Text input/return controls; no native Access SQL runs."""
import argparse
import csv
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_large_indexes import INPUT_HASHES
from compare_numeric_text import floating, spelling


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--native-directory", type=Path)
    args = parser.parse_args()
    controls = 0

    def check(expression, expected, mode, number_format=None, database=None):
        nonlocal controls
        sql = f"SELECT {expression} AS N,VarType({expression}) AS T"
        command = [str(args.cli.resolve()), "query", mode, sql]
        if number_format:
            command += ["--number-format", number_format]
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
    if args.native_directory:
        for suffix, digest in INPUT_HASHES.items():
            database = args.native_directory / ("index-" + suffix)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
            for number_format in ["en-us", "de-de"]:
                for function, expected in [("Hex", "1"), ("Oct", "1"), ("Str", " 1")]:
                    check(f"{function}(CStr(ID))", expected, "run", number_format, database)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
    print(f"Passed {controls} numeric Text input controls; native Access remains unqualified.")


if __name__ == "__main__":
    main()
