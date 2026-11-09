"""Independent Date/Text selector models with inert native-file integration.

Decimal supplies truth/index expectations for bounded finite probes. The script
executes TableCraft, not native Access, and checks unchanged original sources.
"""

import argparse
import datetime as dt
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path
import random

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute


def integer_index(value):
    rounded = int(value.to_integral_value(rounding=ROUND_HALF_EVEN))
    return str(rounded) if 1 <= rounded <= 4 else "NULL"


def numeric_cases():
    values = [Decimal(text) for text in ["0", "-0", "0.25", "-0.25", "1.5", "2.5", "3.5", "4.5", "9007199254740993", "1e25"]]
    rng = random.Random(20261021)
    return values + [Decimal(rng.randrange(-20_000, 20_000)).scaleb(-3) for _ in range(64)]


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
        for profile in ["en-US", "de-DE"]:
            for value in numeric_cases():
                number = format(value, "f")
                text = number.replace(".", ",") if profile == "de-DE" else number
                sql = f"SELECT IIf('{text}',11,22),Switch('{text}',CVErr(2001),True,CVErr(2002)),Choose('{text}',1,2,3,4)"
                truth = bool(value)
                expected = [["11" if truth else "22", f"#ERROR({2001 if truth else 2002})", integer_index(value)]]
                assert execute(args.cli, mode, sql, controls=("--number-format", profile)) == expected
                count += 3
            for text, truth in [("True", True), (" false ", False), ("#TRUE#", True), ("#FALSE#", False)]:
                sql = f"SELECT IIf('{text}',1,0),Switch('{text}',1,True,0)"
                assert execute(args.cli, mode, sql, controls=("--number-format", profile)) == [[str(int(truth))] * 2]
                count += 2
            for sql in [
                "SELECT IIf('bad',1,1/0)", "SELECT Switch('bad',1,True,1/0)",
                "SELECT Choose('bad',1,1/0)", "SELECT IIf(CVErr(0),1,0)",
                "SELECT Switch(True,1,CVErr(0),2)", "SELECT Choose(CVErr(1),1)",
            ]:
                execute(args.cli, mode, sql, controls=("--number-format", profile), success=False)
                count += 1
        for serial in ["-1.25", "-0.25", "0", "0.25", "1.5", "2.5", "3.5"]:
            value = Decimal(serial)
            sql = f"SELECT IIf(CDate({serial}),1,0),Switch(CDate({serial}),1,True,0),Choose(CDate({serial}),1,2,3,4)"
            assert execute(args.cli, mode, sql) == [[str(int(bool(value))), str(int(bool(value))), integer_index(value)]]
            count += 3
        for sql in ["SELECT IIf('1',1,0)", "SELECT Switch('1',1)", "SELECT Choose('1',1)"]:
            execute(args.cli, mode, sql, success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        references = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            identity = int(fields[1].removeprefix("I32:"))
            stamp = None if fields[-1] == "N" else dt.datetime.fromisoformat(fields[-1].removeprefix("Y"))
            truth = stamp is not None and stamp != dt.datetime(1899, 12, 30)
            references.append((identity, truth))
        sql = "SELECT ID,IIf(Stamp,1,0),Switch(Stamp,1,True,0) FROM ScalarSamples ORDER BY ID"
        assert execute(args.cli, "run", sql, database) == [[str(identity), str(int(truth)), str(int(truth))] for identity, truth in references]
        count += 2 * len(references)
        for profile in ["en-US", "de-DE"]:
            sql = "SELECT ID,IIf(CStr(ID),ID,0),Choose(CStr(ID),ID,ID*10,ID*100) FROM ScalarSamples ORDER BY ID"
            expected = [[str(identity), str(identity), str(identity * 10 ** (identity - 1))] for identity, _ in references]
            assert execute(args.cli, "run", sql, database, controls=("--number-format", profile)) == expected
            count += 2 * len(references)
            sql = "SELECT DSum('IIf(CStr(ID),1,0)','ScalarSamples')"
            assert execute(args.cli, "run", sql, database, controls=("--number-format", profile)) == [[str(len(references))]]
            count += 1
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent selector/date/Text/eager/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
