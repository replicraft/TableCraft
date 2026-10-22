"""Original EuroConvert controls using independent Decimal arithmetic and ECB rates.

Jackcess getters provide original stored Currency inputs without evaluating SQL.
These models do not establish Microsoft Access function or regional conformance.
"""

import argparse
import csv
from decimal import Decimal, ROUND_HALF_UP, localcontext
import hashlib
from io import StringIO
import math
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, run

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
RATES = {
    "ATS": ("13.7603", 2), "BEF": ("40.3399", 0), "DEM": ("1.95583", 2),
    "ESP": ("166.386", 0), "FIM": ("5.94573", 2), "FRF": ("6.55957", 2),
    "IEP": ("0.787564", 2), "ITL": ("1936.27", 0), "LUF": ("40.3399", 0),
    "NLG": ("2.20371", 2), "PTE": ("200.482", 0), "EUR": ("1", 2),
}


def model(amount, source, target, full, precision=None):
    if source == target:
        return amount
    euro = amount / Decimal(RATES[source][0])
    if source != "EUR" and target != "EUR" and precision is not None and euro:
        euro = euro.quantize(Decimal(1).scaleb(abs(euro).adjusted() - precision + 1), rounding=ROUND_HALF_UP)
    result = euro * Decimal(RATES[target][0])
    return result if full else result.quantize(Decimal(1).scaleb(-RATES[target][1]), rounding=ROUND_HALF_UP)


def execute(cli, mode, sql, database=None, success=True):
    arguments = [str(cli), "query", mode, sql]
    if database:
        arguments += ["--database", str(database)]
    result = subprocess.run(arguments, capture_output=True, text=True, check=False)
    if not success:
        assert result.returncode != 0 and not result.stdout, (sql, result.stdout)
        return []
    assert result.returncode == 0, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))[1:]


def compare(row, expected):
    assert len(row) == len(expected)
    for text, wanted in zip(row, expected):
        actual = float(text)
        wanted = float(wanted)
        # Decimal and binary64 arithmetic need not have identical last bits.
        tolerance = max(8 * math.ulp(wanted), 1e-12)
        assert abs(actual - wanted) <= tolerance, (actual, wanted, tolerance)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    count = 0
    for mode in ["eval", "run"]:
        for source in RATES:
            for target in RATES:
                for amount in map(Decimal, ["0", "1.2", "-3.7", "1000"]):
                    probes = []
                    expected = []
                    for full, precision in [(False, None), (True, None), (True, 3), (False, 6)]:
                        optional = f",{'True' if full else 'False'}" + (f",{precision}" if precision is not None else "")
                        probes.append(f"EuroConvert({amount},'{source}','{target}'{optional})")
                        expected.append(model(amount, source, target, full, precision))
                    compare(execute(args.cli, mode, "SELECT " + ",".join(probes))[0], expected)
                    count += len(expected)
        for sql in ["SELECT EuroConvert(1.2,'DEM','EUR',)", "SELECT EuroConvert(1.2,'DEM','EUR',,)", "SELECT EuroConvert(1.2,'DEM','EUR',,3)"]:
            compare(execute(args.cli, mode, sql)[0], [Decimal("0.61")])
            count += 1
        for expression in ["EuroConvert(1,'USD','EUR')", "EuroConvert(1,'EUR','GBP')", "EuroConvert(1,'DEM','EUR',True,2)", "EuroConvert(1,'DEM','EUR',Null)", "EuroConvert(1,'DEM','EUR',True,Null)", "EuroConvert(Null,'DEM','EUR',1/0)", "EuroConvert(,1/0,'EUR')", "EuroConvert(*)"]:
            execute(args.cli, mode, "SELECT 1," + expression, success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        expected = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            if fields[5] != "N":
                amount = Decimal(fields[5].removeprefix("C")).scaleb(-4)
                expected.append([Decimal(fields[1].removeprefix("I32:")), model(amount, "DEM", "EUR", False), model(amount, "FRF", "DEM", True, 3)])
        rows = execute(args.cli, "run", "SELECT ID,EuroConvert([Money],'DEM','EUR'),EuroConvert([Money],'FRF','DEM',True,3) FROM ScalarSamples WHERE [Money] Is Not Null ORDER BY ID", database)
        assert len(rows) == len(expected)
        for row, wanted in zip(rows, expected):
            compare(row, wanted)
            count += len(row) - 1
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} EuroConvert rate/triangulation/rounding/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    with localcontext() as context:
        context.prec = 100
        main()
