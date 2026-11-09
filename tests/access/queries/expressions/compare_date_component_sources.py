"""Independent Gregorian controls for coerced date component sources.

Python civil components and exact quarter-day serials supply expectations. Native
getters are read-only field evidence, not native Access expression execution.
"""

import argparse
import datetime as dt
from decimal import Decimal
import hashlib
from pathlib import Path
import random

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute, quoted

FUNCTIONS = ["Year", "Month", "Day", "Hour", "Minute", "Second"]
EPOCH = dt.datetime(1899, 12, 30)


def components(value):
    return list(map(str, [value.year, value.month, value.day,
                         value.hour, value.minute, value.second]))


def projection(value):
    return ",".join(f"{function}({value})" for function in FUNCTIONS)


def spelling(value, profile):
    if profile == "iso":
        return value.isoformat(sep=" ")
    date = (f"{value.month}/{value.day}/{value.year:04}" if profile == "en-us"
            else f"{value.day}.{value.month}.{value.year:04}")
    return f"{date} {value.hour:02}:{value.minute:02}:{value.second:02}"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    values = [dt.datetime(100, 1, 1), dt.datetime(1899, 12, 29, 6),
              dt.datetime(1899, 12, 30, 6), dt.datetime(2024, 2, 29, 23, 59, 59),
              dt.datetime(2025, 1, 1, 12), dt.datetime(9999, 12, 31, 18)]
    rng = random.Random(20261022)
    values += [dt.datetime(100, 1, 1) + dt.timedelta(
        days=rng.randrange(3_000_000), seconds=rng.randrange(86400)
    ) for _ in range(12)]
    count = 0
    for mode in ["eval", "run"]:
        for profile in ["iso", "en-us", "de-de"]:
            controls = ("--date-format", profile, "--number-format",
                        "de-de" if profile == "de-de" else "en-us")
            for value in values:
                sql = "SELECT " + projection(quoted(spelling(value, profile)))
                result = execute(args.cli, mode, sql, controls=controls)
                assert result == [components(value)], (sql, result, value)
                count += 6
            for raw in ["-657434.75", "-1.25", "-0.25", "-0.0", "0.25", "45658.5", "2958465.75"]:
                number = Decimal(raw)
                day = int(number)
                seconds = int(abs(number - day) * 86400)
                value = EPOCH + dt.timedelta(days=day, seconds=seconds)
                token = raw.replace(".", ",") if profile == "de-de" else raw
                for expression in [raw, quoted(token)]:
                    sql = "SELECT " + projection(expression)
                    result = execute(args.cli, mode, sql, controls=controls)
                    assert result == [components(value)], (sql, result, value)
                    count += 6
            assert execute(args.cli, mode, "SELECT " + projection("NULL")) == [["NULL"] * 6]
            count += 6
            for expression in ["Year('nonsense')", "Year(2958466)",
                               "Month('2025-02-30')", "Second(NULL),1/0"]:
                execute(args.cli, mode, "SELECT 99," + expression,
                        controls=controls, success=False)
                count += 1
        execute(args.cli, mode, "SELECT 99,Year('2025-01-01')", success=False)
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
            if fields[-1] != "N":
                references.append((fields[1].removeprefix("I32:"),
                                   dt.datetime.fromisoformat(fields[-1].removeprefix("Y"))))
        for expression in ["CDbl(Stamp)", "CStr(Stamp)"]:
            sql = "SELECT ID," + projection(expression)
            sql += " FROM ScalarSamples WHERE Stamp IS NOT NULL ORDER BY ID"
            for profile in ["en-us", "de-de"]:
                reference = [[identity, *components(value)] for identity, value in references]
                result = execute(args.cli, "run", sql, database,
                                 controls=("--date-format", profile, "--number-format", profile))
                assert result == reference, (database, sql, result, reference)
                count += len(reference) * 6
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent component-source/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
