"""Original standard-code/inspection/selection controls for CVErr query calls.

Integer and Decimal half-even models verify code handling; operational failures
remain failed calls. Native getters supply ID fields without evaluating CVErr.
"""

import argparse
import csv
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261021)
    values = ["0", "1", "255", "256", "32767", "32768", "65535", "-0.5", "2.5", "3.5", "65534.5"]
    values += [str(rng.randrange(65536)) for _ in range(128)]
    count = 0
    for mode in ["eval", "run"]:
        for value in values:
            code = str(Decimal(value).quantize(Decimal(1), rounding=ROUND_HALF_EVEN))
            code = str(int(code))
            expressions = [f"IsError(CVErr({value}))", f"CLng(CVErr(CVErr({value})))", f"VarType(CVErr({value}))", f"TypeName(CVErr({value}))", f"IsNull(CVErr({value}))", f"IsEmpty(CVErr({value}))", f"IsArray(CVErr({value}))"]
            assert execute(args.cli, mode, "SELECT " + ",".join(expressions)) == [["TRUE", code, "10", '"Error"', "FALSE", "FALSE", "FALSE"]]
            count += len(expressions)
            selectors = [f"CLng(IIf(True,CVErr({value}),0))", f"CLng(Choose(2,0,CVErr({value})))", f"CLng(Switch(False,0,True,CVErr({value})))", f"CLng(Nz(CVar(CVErr({value})),0))"]
            assert execute(args.cli, mode, "SELECT " + ",".join(selectors)) == [[code] * len(selectors)]
            count += len(selectors)
        for expression in ["CVErr(2001)", "IsError(CVErr(-1))", "IsError(CVErr(65535.5))", "CVErr(1)+1", "IsError(1/0)", "IsError(IIf(True,CVErr(1),1/0))", "IsError(Choose(1,CVErr(1),1/0))", "IsError(Switch(True,CVErr(1),False,1/0))", "CInt(CVErr(65535))", "CByte(CVErr(256))", "CVErr(Null)", "CVErr(1,1/0)", "CVErr(*)"]:
            execute(args.cli, mode, "SELECT 1," + expression, success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = [line.split("\t")[1].removeprefix("I32:") for line in tagged.splitlines()]
        sql = "SELECT ID,IsError(CVErr(ID)),CLng(CVErr(ID)),VarType(CVar(CVErr(ID))) FROM ScalarSamples ORDER BY ID"
        assert execute(args.cli, "run", sql, database) == [[identity, "TRUE", identity, "10"] for identity in identities]
        count += 3 * len(identities)
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} CVErr code/selection/inspection/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
