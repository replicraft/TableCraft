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
        for number_format, decimal in [("en-us", "."), ("de-de", ",")]:
            for expression, expected in [
                (f"NPV('0{decimal}25',Array(-100,0,125))", -16),
                (f"IRR(Array(-100,0,121),'0{decimal}1')", 0.1),
                (f"MIRR(Array(-100,0,121),'0{decimal}0','0{decimal}0')", 0.1),
            ]:
                check(expression,expected,mode,number_format)
                check(expression,None,mode,error="number-format")
        for expression, error in [
            ("NPV('bad',Array(NULL))", "number-format"),
            ("IRR(Array(NULL),'bad')", "NULL"),
            ("MIRR(Array(NULL),'bad','bad')", "NULL"),
            ("NPV('bad',Array(1/0))", "division by zero"),
        ]:
            check(expression,None,mode,error=error)
        for expression, error in [
            ("Pmt(NULL,'bad','1')", "NULL"),
            ("Pmt('bad',NULL,1/0)", "division by zero"),
            ("EuroConvert(1,'USD','EUR','bad')", "domain"),
        ]:
            check(expression, None, mode, error=error)
    for mode in ["eval", "run"]:
        for name, values, _ in cases:
            arguments = ["" if value is None else "'" + value + "'" if name == "EuroConvert" and index in [1, 2] else value for index, value in enumerate(values)]
            arguments[0] = "CVErr(1)"
            check(name + "(" + ",".join(arguments) + ")", None, mode, error="type mismatch")
            arguments[-1] = "1/0"
            check(name + "(" + ",".join(arguments) + ")", None, mode, error="division by zero")
        for expression, error in [
            ("SLN(NULL,CVErr(1),10)", "NULL"),
            ("SLN(CVErr(1),NULL,10)", "type mismatch"),
            ("SLN(-1,CVErr(1),0)", "type mismatch"),
            ("EuroConvert(1,'USD',CVErr(1))", "domain"),
            ("EuroConvert(1,CVErr(1),'USD')", "type mismatch"),
        ]:
            check(expression, None, mode, error=error)
        check("SLN(CLng(CVErr(1)),0,1)", 1, mode)
    for mode in ["eval", "run"]:
        for expression, error in [
            ("NPV(CVErr(1),Array(1/0))", "division by zero"),
            ("NPV(0,Array(CVErr(1),1/0))", "division by zero"),
            ("IRR(Array(CVErr(1)),1/0)", "division by zero"),
            ("MIRR(Array(CVErr(1)),1/0,0)", "division by zero"),
            ("NPV(CVErr(1),Array(NULL))", "type mismatch"),
            ("IRR(Array(NULL),CVErr(1))", "NULL"),
            ("MIRR(Array(NULL),CVErr(1),0)", "NULL"),
            ("NPV(0,Array(CVErr(0)))", "type mismatch"),
            ("NPV(0,Array(NULL,CVErr(1)))", "NULL"),
        ]:
            check(expression, None, mode, error=error)
        check("NPV(0,Array(CLng(CVErr(1))))", 1, mode)
    if args.native_directory:
        for suffix, digest in INPUT_HASHES.items():
            database = args.native_directory / ("index-" + suffix)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
            for number_format in ["en-us", "de-de"]:
                for expression, expected in [
                    ("SLN(CStr(ID),'0','1')", 1),
                    ("Pmt('0','10',CStr(ID))", -0.1),
                    ("EuroConvert(CStr(ID),'EUR','EUR','1','3')", 1),
                    ("NPV('0',Array(-ID,0,2*ID))", 1),
                    ("IRR(Array(-ID,0,1.21*ID),'0.1')" if number_format == "en-us" else "IRR(Array(-ID,0,1.21*ID),'0,1')", 0.1),
                    ("MIRR(Array(-ID,0,1.21*ID),'0','0')", 0.1),
                ]:
                    check(expression, expected, number_format=number_format, database=database)
            for expression, error in [
                ("SLN(CVErr(ID),0,1)", "type mismatch"),
                ("Pmt(CVErr(ID),1/(ID-1),1)", "division by zero"),
                ("EuroConvert(1,'USD',CVErr(ID))", "domain"),
            ]:
                check(expression, None, database=database, error=error)
            for expression, error in [
                ("NPV(CVErr(ID),Array(1/(ID-1)))", "division by zero"),
                ("IRR(Array(CVErr(ID)),1/(ID-1))", "division by zero"),
                ("NPV(0,Array(CVErr(ID)))", "type mismatch"),
            ]:
                check(expression, None, database=database, error=error)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
    print(f"Passed {controls} independent financial Text controls; native Access remains unqualified.")


if __name__ == "__main__":
    main()
