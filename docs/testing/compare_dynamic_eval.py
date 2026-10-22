"""Original closed-Eval arithmetic/context/refusal/native-field controls.

Python integer/Boolean/civil models supply expectations. Native getters supply
IDs without evaluating database expressions; Microsoft Access Eval is not run.
"""

import argparse
import csv
from datetime import date, timedelta
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256


def quote(text):
    return "'" + text.replace("'", "''") + "'"


def execute(cli, mode, sql, database=None, controls=(), success=True):
    command = [str(cli), "query", mode, sql, "--number-format", "invariant", *controls]
    if database:
        command += ["--database", str(database)]
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if not success:
        assert result.returncode != 0 and not result.stdout, (sql, result.stdout)
        return []
    assert result.returncode == 0, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))[1:]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261021)
    triples = [(0, 0, 0), (-1, 2, 3)]
    triples += [tuple(rng.randrange(-1000, 1001) for _ in range(3)) for _ in range(126)]
    count = 0
    for mode in ["eval", "run"]:
        for a, b, c in triples:
            arithmetic = f"({a}+{b})*({c})"
            nested = f"Eval({quote(arithmetic)})"
            low, high = sorted([b, c])
            expressions = [
                f"Eval({quote(arithmetic)})",
                f"Eval({quote(nested)})",
                f"Eval({quote(f'{a} Between {low} And {high}')})",
                f"Eval({quote(f'{a} In ({b},{c})')})",
            ]
            expected = [str((a + b) * c)] * 2 + ["TRUE" if low <= a <= high else "FALSE", "TRUE" if a in [b, c] else "FALSE"]
            assert execute(args.cli, mode, "SELECT " + ",".join(expressions)) == [expected]
            count += len(expected)
        expected_date = date(2026, 10, 21) + timedelta(days=10)
        sql = "SELECT Eval('15+3'),Eval('Chr$(65)'),Eval('CVErr(2001)'),Eval('Date()+Month(Date())'),Eval(15)"
        expected = ["18", '"A"', "#ERROR(2001)", f"#{expected_date.isoformat()} 00:00:00.000000000#", "15"]
        assert execute(args.cli, mode, sql, controls=("--clock", "2026-10-21T13:14:15")) == [expected]
        count += len(expected)
        for expression in [
            "Eval('1/0')", "Eval('1;SELECT 2')", "Eval('(SELECT 1)')", "Eval('[Secret]')",
            "Eval('Shell(''x'')')", "Eval('DSum(''N'',''Items'')')", "Eval(Null)",
            "Eval(*)", "Eval()", "Eval('1',1/0)", "Eval('15+3'),1/0",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = [int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines()]
        sql = "SELECT ID,Eval(CStr(ID) & '+17'),IsError(Eval('CVErr(' & CStr(ID) & ')')) FROM ScalarSamples ORDER BY ID"
        assert execute(args.cli, "run", sql, database) == [[str(identity), str(identity + 17), "TRUE"] for identity in identities]
        count += 2 * len(identities)
        execute(args.cli, "run", "SELECT Eval('ID') FROM ScalarSamples", database, success=False)
        count += 1
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent Eval arithmetic/context/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
