"""Independent Decimal/week-enumeration controls for numeric week options.

No SQL/VBA reference implementation is executed. Native getters supply dates and
IDs only, with input and reference JAR hashes checked before/after each run.
"""

import argparse
import datetime as dt
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute, quoted
from compare_week_context import boundaries, weekday, week_number


def rounded(value):
    return int(Decimal(value).to_integral_value(rounding=ROUND_HALF_EVEN))


def option(value, profile, carrier):
    if carrier == "text":
        return quoted(value.replace(".", ",") if profile == "de-de" else value)
    return f"CDate({value})"


def expected(value, other, first, rule):
    day = weekday(value, first)
    week = week_number(value, first, rule)
    return [str(day), str(week), str(day),
            str(boundaries(value, other, first)), quoted(f"{day} {week}")]


def projection(value, other, first, rule):
    return ",".join([
        f"DatePart('w',{value},{first},{rule})",
        f"DatePart('ww',{value},{first},{rule})",
        f"Weekday({value},{first})",
        f"DateDiff('ww',{value},{other},{first},{rule})",
        f"Format({value},'w ww',{first},{rule})",
    ])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    dates = [dt.date(2003, 12, 29), dt.date(2003, 12, 30),
             dt.date(2021, 1, 1), dt.date(2025, 1, 1)]
    settings = [("1.5", "1.5"), ("2.5", "2.5"), ("3.5", "2.5"),
                ("6.5", "0.5"), ("-0.5", "-0.5"), ("7.5", "1"),
                ("1", "3.5")]
    count = 0
    for profile in ["en-us", "de-de"]:
        controls = ("--number-format", profile, "--first-day-of-week", "2",
                    "--first-week-of-year", "3")
        for carrier in ["text", "date"]:
            for first_text, rule_text in settings:
                first, rule = rounded(first_text), rounded(rule_text)
                first = first or 2
                rule = rule or 3
                valid = 1 <= first <= 7 and 1 <= rule <= 3
                first_sql = option(first_text, profile, carrier)
                rule_sql = option(rule_text, profile, carrier)
                for value in dates:
                    other = value + dt.timedelta(days=9)
                    sql = "SELECT " + projection(
                        f"#{value}#", f"#{other}#", first_sql, rule_sql)
                    for mode in ["eval", "run"]:
                        result = execute(args.cli, mode, sql, controls=controls,
                                         success=valid)
                        if valid:
                            reference = expected(value, other, first, rule)
                            assert result == [reference], (sql, result, reference)
                        count += 5 if valid else 1
        for mode in ["eval", "run"]:
            for expression in [
                "DatePart('ww',NULL,'8',1/0)",
                "DateDiff('ww',NULL,NULL,'8',1/0)",
                "Format(NULL,'w','8',1/0)",
                "Weekday(NULL,1/0)",
                "DatePart('ww',NULL,'bad')",
                "Format(NULL,'w','bad',NULL)",
            ]:
                execute(args.cli, mode, "SELECT 99," + expression,
                        controls=controls, success=False)
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
                identity = int(fields[1].removeprefix("I32:"))
                date = dt.datetime.fromisoformat(fields[-1].removeprefix("Y")).date()
                references.append((identity, date))
        other = dt.date(2025, 1, 10)
        sql = "SELECT ID," + projection("Stamp", f"#{other}#", "CStr(ID)", "'2.5'")
        sql += " FROM ScalarSamples WHERE Stamp IS NOT NULL ORDER BY ID"
        reference = [[str(identity), *expected(date, other, identity, 2)]
                     for identity, date in references]
        result = execute(args.cli, "run", sql, database,
                         controls=("--number-format", "en-us"))
        assert result == reference, (database, result, reference)
        count += len(reference) * 5
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent numeric/week/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
