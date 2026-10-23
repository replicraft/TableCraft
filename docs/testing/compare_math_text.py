"""Original numeric Text/math models; no Access or reference SQL executes."""
import argparse
import csv
import hashlib
from io import StringIO
import math
from pathlib import Path
import struct
import subprocess

from compare_large_indexes import INPUT_HASHES
from compare_random import model


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--native-directory", type=Path)
    args = parser.parse_args()
    controls = 0
    _, random_value = model(1234, -1)

    def check(expression, expected, tag, mode="run", number_format=None,
              database=None, error=None):
        nonlocal controls
        sql = f"SELECT {expression} AS N,VarType({expression}) AS T"
        command = [str(args.cli.resolve()), "query", mode, sql, "--random-seed", "1234"]
        if number_format:
            command += ["--number-format", number_format]
        if database:
            sql += " FROM IndexedRows WHERE ID=1"
            command[3] = sql
            command += [
                "--database", str(database), "--index", "IndexedRows", "IdAsc",
                "--native-bytes", "67108864", "--native-rows", "100000",
                "--native-steps", "10000000", "--execution-steps", "10000000",
            ]
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        if expected is None:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert error in result.stderr, (sql, result.stderr, error)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[0] == ["N", "T"] and len(rows) == 2, (sql, rows)
            assert rows[1][1] == str(tag), (sql, rows, tag)
            actual = float(rows[1][0])
            if tag == 4:
                assert struct.pack("<f", actual) == struct.pack("<f", expected), (
                    sql, rows, expected
                )
            else:
                assert math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12), (
                    sql, rows, expected
                )
        controls += 1

    for mode in ["eval", "run"]:
        for number_format, decimal in [("en-us", "."), ("de-de", ",")]:
            # Exact special values and half-even ties have elementary results.
            for expression, expected, tag in [
                (f"Abs('-2{decimal}5')", 2.5, 5),
                (f"Sgn('-2{decimal}5')", -1, 2),
                (f"Int('-2{decimal}5')", -3, 5),
                (f"Fix('-2{decimal}5')", -2, 5),
                (f"Sqr('2{decimal}25')", 1.5, 5),
                ("Exp('0')", 1, 5), ("Log('1')", 0, 5),
                ("Sin('0')", 0, 5), ("Cos('0')", 1, 5),
                ("Tan('0')", 0, 5), ("Atn('0')", 0, 5),
                (f"Round('2{decimal}5','0')", 2, 5),
                (f"RGB('2{decimal}5','1{decimal}5','3{decimal}5')", 262658, 3),
                (f"QBColor('2{decimal}5')", 32768, 3),
                ("Rnd('-1')", random_value, 4),
            ]:
                check(expression, expected, tag, mode, number_format)
                check(expression, None, None, mode, error="number-format")
        for expression, error in [
            ("Round('bad','29')", "domain"),
            ("Round(NULL,'29')", "domain"),
            ("RGB(-1,'bad',0)", "domain"),
            ("QBColor('16')", "domain"),
            ("RGB('2147483648',0,0)", "overflow"),
            ("Round('bad',1/0)", "division by zero"),
        ]:
            check(expression, None, None, mode, "en-us", error=error)
    if args.native_directory:
        for suffix, digest in INPUT_HASHES.items():
            database = args.native_directory / ("index-" + suffix)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
            for number_format in ["en-us", "de-de"]:
                for expression, expected, tag in [
                    ("Abs(CStr(-ID))", 1, 5),
                    ("RGB(CStr(ID),'0','0')", 1, 3),
                    ("Round(CCur(ID),'0')", 1, 6),
                    ("Rnd(CStr(-ID))", random_value, 4),
                ]:
                    check(expression, expected, tag, number_format=number_format,
                          database=database)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
    print(f"Passed {controls} independent math Text controls; native Access remains unqualified.")


if __name__ == "__main__":
    main()
