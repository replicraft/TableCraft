"""Independent Gregorian/Decimal models for coerced date interval calls.

Reference getters only read native fields. Python computes calendar additions,
parts and boundaries; no native Access SQL or copied implementation is executed.
"""

import argparse
import calendar
import datetime as dt
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_date_component_sources import components, spelling
from compare_error_messages import execute, quoted
from compare_format_serials import serial
from compare_week_context import boundaries, weekday, week_number

INTERVALS = ["yyyy", "q", "m", "y", "d", "w", "ww", "h", "n", "s"]


def add(value, interval, count):
    number = int(Decimal(count).to_integral_value(rounding=ROUND_HALF_EVEN))
    if interval in ["yyyy", "q", "m"]:
        months = number * {"yyyy": 12, "q": 3, "m": 1}[interval]
        year, month = divmod(value.year * 12 + value.month - 1 + months, 12)
        month += 1
        return value.replace(year=year, month=month,
                             day=min(value.day, calendar.monthrange(year, month)[1]))
    unit = {"y": "days", "d": "days", "w": "days", "ww": "weeks",
            "h": "hours", "n": "minutes", "s": "seconds"}[interval]
    return value + dt.timedelta(**{unit: number})


def part(value, interval):
    return {"yyyy": value.year, "q": (value.month - 1) // 3 + 1,
            "m": value.month, "y": value.timetuple().tm_yday, "d": value.day,
            "w": weekday(value.date(), 2), "ww": week_number(value.date(), 2, 2),
            "h": value.hour, "n": value.minute, "s": value.second}[interval]


def difference(first, second, interval):
    if interval == "yyyy":
        return second.year - first.year
    if interval == "q":
        return (second.year * 4 + (second.month - 1) // 3
                - first.year * 4 - (first.month - 1) // 3)
    if interval == "m":
        return second.year * 12 + second.month - first.year * 12 - first.month
    if interval == "ww":
        return boundaries(first.date(), second.date(), 2)
    days = (second.date() - first.date()).days
    if interval == "w":
        return (1 if days >= 0 else -1) * (abs(days) // 7)
    if interval in ["d", "y"]:
        return days
    resolution = {"h": {"minute": 0, "second": 0}, "n": {"second": 0}, "s": {}}[interval]
    seconds = int((second.replace(**resolution) - first.replace(**resolution)).total_seconds())
    return seconds // {"h": 3600, "n": 60, "s": 1}[interval]


def source(value, carrier, profile):
    if carrier == "calendar":
        return quoted(spelling(value, profile))
    number = str(serial(value))
    if carrier == "text":
        return quoted(number.replace(".", ",") if profile == "de-de" else number)
    return number


def check(cli, mode, controls, cases, database=None):
    count = 0
    for offset in range(0, len(cases), 6):
        batch = cases[offset:offset + 6]
        sql = "SELECT " + ",".join(expression for expression, _ in batch)
        result = execute(cli, mode, sql, database, controls=controls)
        expected = [[str(value) for _, value in batch]]
        assert result == expected, (sql, result, expected)
        count += len(batch)
    return count


def added_components(expression, expected):
    return [(f"{function}({expression})", value) for function, value in zip(
        ["Year", "Month", "Day", "Hour", "Minute", "Second"], components(expected))]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    values = [dt.datetime(1899, 12, 29, 6), dt.datetime(2003, 12, 29, 12),
              dt.datetime(2021, 1, 1, 18), dt.datetime(2024, 1, 31, 6),
              dt.datetime(2024, 2, 29, 12)]
    count = 0
    for mode in ["eval", "run"]:
        for profile in ["iso", "en-us", "de-de"]:
            controls = ("--date-format", profile, "--number-format",
                        "de-de" if profile == "de-de" else "en-us")
            for carrier in ["numeric", "text", "calendar"]:
                cases = []
                for value in values:
                    value_sql = source(value, carrier, profile)
                    other = value + dt.timedelta(days=9, hours=1)
                    other_sql = source(other, carrier, profile)
                    for interval in INTERVALS:
                        cases.append((f"DatePart('{interval}',{value_sql},'2','2')", part(value, interval)))
                        first = 1 if interval == "w" else 2
                        diff = f"DateDiff('{interval}',{value_sql},{other_sql},'{first}','2')"
                        reverse = f"DateDiff('{interval}',{other_sql},{value_sql},'{first}','2')"
                        expected_diff = difference(value, other, interval)
                        cases += [(diff, expected_diff), (reverse, -expected_diff)]
                        for number in ["-1.5", "2.5"]:
                            option = number.replace(".", ",") if profile == "de-de" else number
                            expression = f"DateAdd('{interval}',{quoted(option)},{value_sql})"
                            cases += added_components(expression, add(value, interval, number))
                count += check(args.cli, mode, controls, cases)
            for expression in [
                "DateAdd('bad','bad','bad')", "DatePart('d','bad',8)",
                "DateDiff('w','bad','bad',2)", "DateDiff('d',NULL,'bad')",
                "DateDiff('d','bad',NULL)", "DateAdd('bad','bad',1/0)",
                "DatePart('bad',1/0)", "DateDiff('bad',NULL,1/0)",
            ]:
                execute(args.cli, mode, "SELECT 99," + expression, controls=controls, success=False)
                count += 1
            for expression in ["DateAdd('d',1,NULL)", "DatePart('d',NULL)",
                               "DateDiff('d',NULL,'2025-01-01')"]:
                assert execute(args.cli, mode, "SELECT " + expression, controls=controls) == [["NULL"]]
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
        for profile in ["en-us", "de-de"]:
            for carrier in ["CDbl(Stamp)", "CStr(Stamp)"]:
                sql = (f"SELECT ID,DatePart('d',{carrier}),"
                       f"DateDiff('h',{carrier},DateAdd('d','1',{carrier})),"
                       f"Day(DateAdd('m','1',{carrier})) "
                       "FROM ScalarSamples WHERE Stamp IS NOT NULL ORDER BY ID")
                expected = [[identity, str(value.day), "24", str(add(value, "m", "1").day)]
                            for identity, value in references]
                result = execute(args.cli, "run", sql, database,
                                 controls=("--date-format", profile, "--number-format", profile))
                assert result == expected, (sql, result, expected)
                count += len(expected) * 3
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent date-interval-source/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
