"""Independent Decimal/calendar checks for named numeric DATE formatting.

Quarter-day inputs are exactly representable as binary64 and avoid conflating
formatter display rounding with DATE calendar rounding. Native getters only supply
date fields, not reference SQL execution. No fixture or implementation is copied.
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
from compare_format_dates import literal
from compare_format_serials import serial
from compare_styled_number import expected


def check_cases(cli, mode, profile, cases):
    count = 0
    for start in range(0, len(cases), 6):
        batch = cases[start:start + 6]
        sql = "SELECT " + ",".join(expression for expression, _ in batch)
        reference = [[quoted(value) for _, value in batch]]
        result = execute(cli, mode, sql, controls=("--number-format", profile))
        assert result == reference, (sql, result, reference)
        count += len(batch)
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    values = [
        dt.datetime(100, 1, 1), dt.datetime(100, 1, 1, 18),
        dt.datetime(1899, 12, 29, 6), dt.datetime(1899, 12, 30),
        dt.datetime(1899, 12, 30, 6), dt.datetime(2025, 1, 1, 12),
        dt.datetime(9999, 12, 31, 18),
    ]
    rng = random.Random(20261022)
    values += [dt.datetime(100, 1, 1) + dt.timedelta(
        days=rng.randrange(3_000_000), hours=rng.randrange(4) * 6
    ) for _ in range(24)]
    count = 0
    for mode in ["eval", "run"]:
        for profile in ["invariant", "en-us", "de-de"]:
            kinds = ["FormatNumber", "FormatPercent"]
            if profile != "invariant":
                kinds.append("FormatCurrency")
            cases = []
            for value in values:
                for kind in kinds:
                    for places in [0, 2, 4]:
                        expression = f"{kind}({literal(value)},{places})"
                        cases.append((expression, expected(serial(value), kind, profile, places)))
            for value in ["-1.25", "-0.25", "-0.0", "0.0", "0.25"]:
                for kind in kinds:
                    expression = f"{kind}(CDate({value}))"
                    cases.append((expression, expected(Decimal(value), kind, profile)))
            count += check_cases(args.cli, mode, profile, cases)
            for expression in ["FormatNumber(NULL)", "FormatCurrency(NULL,1/0)",
                               "FormatPercent(#2025-01-01#),1/0"]:
                execute(args.cli, mode, "SELECT 99," + expression,
                        controls=("--number-format", profile), success=False)
                count += 1
        execute(args.cli, mode, "SELECT 99,FormatNumber(#2025-01-01#)", success=False)
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
        sql = ("SELECT ID,FormatNumber(Stamp),FormatPercent(Stamp),FormatCurrency(Stamp) "
               "FROM ScalarSamples WHERE Stamp IS NOT NULL ORDER BY ID")
        for profile in ["en-us", "de-de"]:
            reference = [[identity, *[quoted(expected(serial(value), kind, profile))
                          for kind in ["FormatNumber", "FormatPercent", "FormatCurrency"]]]
                         for identity, value in references]
            result = execute(args.cli, "run", sql, database,
                             controls=("--number-format", profile))
            assert result == reference, (database, result, reference)
            count += len(reference) * 3
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent numeric-DATE/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
