"""Check Gregorian Format masks against independent Python calendar models.

Optional original ScalarFixture files supply stored timestamps through reviewed
Jackcess getters. This does not execute Microsoft Access or qualify Windows NLS.
"""

import argparse
import csv
import datetime as dt
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import PROFILES, RECIPES, run
from compare_cstr import date_text, spelling
from compare_date_format import EN_MONTHS, EN_WEEKDAYS, DE_MONTHS, DE_WEEKDAYS, expected as named
from compare_week_context import weekday, week_number

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
DE_SHORT_MONTHS = "Jan Feb Mär Apr Mai Jun Jul Aug Sep Okt Nov Dez".split()
DE_SHORT_DAYS = "Mo Di Mi Do Fr Sa So".split()


def literal(value):
    return f"#{value.year:04}-{value.month:02}-{value.day:02} {value.hour:02}:{value.minute:02}:{value.second:02}.{value.microsecond:06}#"


def sql_text(text):
    return "'" + text.replace("'", "''") + "'"


def rounded(value):
    if value.microsecond >= 500000:
        value += dt.timedelta(seconds=1)
    return value.replace(microsecond=0)


def models(value, profile):
    value = rounded(value)
    months = DE_MONTHS if profile == "de-de" else EN_MONTHS
    days = DE_WEEKDAYS if profile == "de-de" else EN_WEEKDAYS
    short_months = DE_SHORT_MONTHS if profile == "de-de" else [name[:3] for name in EN_MONTHS]
    short_days = DE_SHORT_DAYS if profile == "de-de" else [name[:3] for name in EN_WEEKDAYS]
    marker = "AM" if value.hour < 12 else "PM"
    hour = value.hour % 12 or 12
    separator = {"iso": "-", "en-us": "/", "de-de": "."}[profile]
    return [
        ("yyyy-mm-dd", f"{value.year:04}-{value.month:02}-{value.day:02}"),
        ("d dd m mm yy yyyy q y", f"{value.day} {value.day:02} {value.month} {value.month:02} {value.year % 100:02} {value.year:04} {(value.month - 1) // 3 + 1} {value.timetuple().tm_yday}"),
        ("hh:nn:ss", f"{value.hour:02}:{value.minute:02}:{value.second:02}"),
        ("h:m:s", f"{value.hour}:{value.minute}:{value.second}"),
        ("hh:mm:ss am/pm", f"{hour:02}:{value.minute:02}:{value.second:02} {marker.lower()}"),
        ("h:nn A/P", f"{hour}:{value.minute:02} {marker[0]}"),
        ("dddd, mmm d yyyy", f"{days[value.weekday()]}, {short_months[value.month - 1]} {value.day} {value.year:04}"),
        ("mmm mmmm ddd dddd", f"{short_months[value.month - 1]} {months[value.month - 1]} {short_days[value.weekday()]} {days[value.weekday()]}"),
        ("yyyy/mm/dd", f"{value.year:04}{separator}{value.month:02}{separator}{value.day:02}"),
        ("ddddd", named(value, profile, 2)),
        ("dddddd", named(value, profile, 1)),
        ("c", date_text(value, profile)),
        ("ttttt", named(value, profile, 3)),
        ('yyyy" 🦀; AM/PM "mm', f"{value.year:04} 🦀; AM/PM {value.month:02}"),
        ("General Date", named(value, profile, 0)),
        ("Long Date", named(value, profile, 1)),
        ("Short Date", named(value, profile, 2)),
        ("Long Time", named(value, profile, 3)),
        ("Short Time", named(value, profile, 4)),
        ("Medium Date", f"{value.day:02}-{short_months[value.month - 1]}-{value.year % 100:02}"),
    ] + ([("Medium Time", f"{hour:02}:{value.minute:02} {marker}"), ("hh:nn AMPM", f"{hour:02}:{value.minute:02} {marker}")] if profile == "en-us" else [])


def execute(cli, mode, sql, profile=None, options=(), succeeds=True):
    arguments = [str(cli), "query", mode, sql, *map(str, options)]
    if profile is not None:
        arguments += ["--date-format", profile]
    result = subprocess.run(arguments, capture_output=True, text=True, check=False)
    if not succeeds:
        assert result.returncode != 0 and not result.stdout, (sql, result.stdout)
        return []
    assert result.returncode == 0, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))[1:]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--directory", type=Path)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    if bool(args.jar) != bool(args.directory):
        parser.error("--jar and --directory must be supplied together")
    if args.generate and args.directory is None:
        parser.error("--generate requires --directory and --jar")
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for family, suffix in PROFILES:
            run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "ScalarFixture.java", args.directory / f"Scalar-{family}.{suffix}", family)

    rng = random.Random(20261021)
    values = [dt.datetime(1899, 12, 30), dt.datetime(1899, 12, 30, 6), dt.datetime(1993, 1, 27, 17, 4, 23), dt.datetime(2024, 2, 29, 23, 59, 59, 750000), dt.datetime(100, 1, 1), dt.datetime(9999, 12, 31, 23, 59, 59)]
    values += [dt.datetime(1900, 1, 1) + dt.timedelta(days=rng.randrange(73000), seconds=rng.randrange(86400)) for _ in range(96)]
    count = 0
    for mode in ["eval", "run"]:
        for profile in ["iso", "en-us", "de-de"]:
            for value in values:
                cases = models(value, profile)
                # Each SELECT shares the CLI's existing cumulative work ceiling.
                # Batching preserves all probes without increasing execution limits.
                for start in range(0, len(cases), 6):
                    batch = cases[start:start + 6]
                    sql = "SELECT " + ",".join(f"Format({literal(value)},{sql_text(mask)})" for mask, _ in batch)
                    assert execute(args.cli, mode, sql, profile) == [[spelling(expected) for _, expected in batch]], (mode, profile, value, batch)
                    count += len(batch)
        for first in range(1, 8):
            for rule in range(1, 4):
                dates = [dt.date(2003, 12, 29), dt.date(2003, 12, 30), dt.date(2018, 12, 31), dt.date(2021, 1, 1), dt.date(2024, 12, 31)]
                for date in dates:
                    expected = f"{weekday(date, first)} {week_number(date, first, rule)}"
                    sql = f"SELECT Format(#{date}#,'w ww',0,0)"
                    assert execute(args.cli, mode, sql, options=["--first-day-of-week", first, "--first-week-of-year", rule]) == [[spelling(expected)]]
                    count += 1
        for expression in ["Format(#1993-01-27#,'dddd')", "Format(#1993-01-27#,'m/d/yyyy')", "Format(#1993-01-27#,'hh:nn')", "Format(1,'yyyyy')", "Format(1,'yyyy;yyyy')", "Format(Null,'yyyy',1/0)"]:
            execute(args.cli, mode, "SELECT 1," + expression, succeeds=False)
            count += 1
        for expression in ["Format(#1993-01-27#,'AMPM')", "Format(#1993-01-27#,'Medium Time')"]:
            execute(args.cli, mode, "SELECT 1," + expression, profile="de-de", succeeds=False)
            count += 1
        assert execute(args.cli, mode, "SELECT Format(#1993-01-27#,'yyyy-mm-dd')") == [[spelling("1993-01-27")]]
        count += 1

    if args.directory:
        for family, suffix in PROFILES:
            database = args.directory / f"Scalar-{family}.{suffix}"
            before = hashlib.sha256(database.read_bytes()).hexdigest()
            tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "ScalarSamples")
            decoded = []
            for line in tagged.splitlines():
                fields = line.split("\t")
                identity = fields[1].removeprefix("I32:")
                value = None if fields[-1] == "N" else dt.datetime.fromisoformat(fields[-1].removeprefix("Y"))
                decoded.append((identity, value))
            for profile in ["iso", "en-us", "de-de"]:
                masks = ["yyyy-mm-dd", "hh:mm:ss am/pm", "dddd, mmm d yyyy", "w ww"]
                expected = []
                for identity, value in decoded:
                    if value is None:
                        cells = [""] * len(masks)
                    else:
                        mapping = dict(models(value, profile))
                        cells = [mapping[mask] for mask in masks[:-1]] + [f"{weekday(value.date(), 1)} {week_number(value.date(), 1, 1)}"]
                    expected.append([identity, *map(spelling, cells)])
                sql = "SELECT ID," + ",".join(f"Format(Stamp,{sql_text(mask)})" for mask in masks) + " FROM ScalarSamples ORDER BY ID"
                assert execute(args.cli, "run", sql, profile, ["--database", database]) == expected
                count += len(masks) * len(expected)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == before
            print(f"Unchanged {family}: {database.stat().st_size} bytes SHA-256 {before}")
    print(f"Passed {count} independent date Format/calendar/week/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
