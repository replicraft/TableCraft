"""Original explicit-policy financial Text controls; native Access is not an oracle."""
import argparse
import csv
import hashlib
from io import StringIO
import math
from pathlib import Path
import subprocess

from compare_large_indexes import INPUT_HASHES


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--native-directory", type=Path)
    args = parser.parse_args()
    controls = 0

    def check(expression, expected, mode="run", number_format=None, database=None, error=None):
        nonlocal controls
        sql = f"SELECT {expression} AS N,VarType({expression}) AS T"
        command = [str(args.cli.resolve()), "query", mode, sql]
        if number_format:
            command += ["--number-format", number_format]
        if database:
            sql += " FROM IndexedRows WHERE ID=1"
            command[3] = sql
            command += ["--database", str(database), "--index", "IndexedRows", "IdAsc",
                        "--native-bytes", "67108864", "--native-rows", "100000",
                        "--native-steps", "10000000", "--execution-steps", "10000000"]
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        if expected is None:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert error in result.stderr, (sql, result.stderr, error)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[0] == ["N", "T"] and len(rows) == 2 and rows[1][1] == "5", (sql, rows)
            assert math.isclose(float(rows[1][0]), expected, rel_tol=1e-12, abs_tol=1e-12), (sql, rows, expected)
        controls += 1

    # Zero-rate ledgers, elementary depreciation and identity currency conversion
    # provide expected results independently of implementation financial formulas.
    cases = [
        ("Pmt", ["0", "10", "1000"], -100),
        ("PV", ["0", "10", "-100"], 1000),
        ("FV", ["0", "10", "-100"], 1000),
        ("NPer", ["0", "-100", "1000"], 10),
        ("IPmt", ["0", "1", "10", "1000"], 0),
        ("PPmt", ["0", "1", "10", "1000"], -100),
        ("SLN", ["1000", "0", "10"], 100),
        ("SYD", ["1100", "0", "10", "1"], 200),
        ("DDB", ["1000", "0", "10", "1"], 200),
        ("Rate", ["10", "-100", "1000", None, None, "0"], 0),
        ("EuroConvert", ["1.5", "EUR", "EUR", "1", "3.5"], 1.5),
    ]
    for mode in ["eval", "run"]:
        for number_format in ["en-us", "de-de"]:
            for name, values, expected in cases:
                arguments = ["" if value is None else "'" + (value.replace(".", ",") if number_format == "de-de" else value) + "'" for value in values]
                expression = name + "(" + ",".join(arguments) + ")"
                check(expression, expected, mode, number_format)
                check(expression, None, mode, error="number-format")
        for expression, error in [
            ("Pmt(NULL,'bad','1')", "NULL"),
            ("Pmt('bad',NULL,1/0)", "division by zero"),
            ("EuroConvert(1,'USD','EUR','bad')", "domain"),
        ]:
            check(expression, None, mode, error=error)
    if args.native_directory:
        for suffix, digest in INPUT_HASHES.items():
            database = args.native_directory / ("index-" + suffix)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
            for number_format in ["en-us", "de-de"]:
                for expression, expected in [
                    ("SLN(CStr(ID),'0','1')", 1),
                    ("Pmt('0','10',CStr(ID))", -0.1),
                    ("EuroConvert(CStr(ID),'EUR','EUR','1','3')", 1),
                ]:
                    check(expression, expected, number_format=number_format, database=database)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
    print(f"Passed {controls} independent financial Text controls; native Access remains unqualified.")


if __name__ == "__main__":
    main()
