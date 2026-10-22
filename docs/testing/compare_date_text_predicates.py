"""Independent Gregorian/serial predicate grids and inert native date getters.

Only TableCraft executes SQL. Explicit date policy models do not qualify native
Access declared/Variant typing, NLS or optimizer/error-order behavior.
"""

import argparse
import datetime as dt
import hashlib
from pathlib import Path

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute


def spelling(value, profile):
    year = f"{value.year:04}"
    if profile == "iso":
        date = f"{year}-{value.month:02}-{value.day:02}"
    elif profile == "en-us":
        date = f"{value.month:02}/{value.day:02}/{year}"
    else:
        date = f"{value.day:02}.{value.month:02}.{year}"
    return date + f" {value.hour:02}:{value.minute:02}:{value.second:02}"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    count = 0
    moments = [
        dt.datetime(*parts)
        for parts in [
            (100, 1, 2, 0),
            (1600, 2, 29, 6),
            (1899, 12, 29, 6),
            (1899, 12, 30, 6),
            (2000, 2, 29, 12),
            (2025, 1, 1, 12),
            (2030, 1, 1, 0),
            (9999, 12, 30, 18),
        ]
    ]
    for profile in ["iso", "en-us", "de-de"]:
        for moment in moments:
            literal = spelling(moment, "iso")
            text = spelling(moment, profile)
            first = spelling(moment - dt.timedelta(days=1), profile)
            last = spelling(moment + dt.timedelta(days=1), profile)
            sql = (
                f"SELECT #{literal}#='{text}','{text}'=#{literal}#,"
                f"#{literal}#<'{last}',#{literal}# BETWEEN '{first}' AND '{last}',"
                f"#{literal}# IN ('{text}')"
            )
            for mode in ["eval", "run"]:
                assert execute(
                    args.cli, mode, sql, controls=("--date-format", profile)
                ) == [["TRUE"] * 5]
                count += 5
        for mode in ["eval", "run"]:
            for sql in [
                "SELECT #2025-01-01#='bad'",
                "SELECT #2025-01-01# IN ('2025-01-01',1/0)",
            ]:
                execute(
                    args.cli, mode, sql,
                    controls=("--date-format", profile), success=False,
                )
                count += 1
    for mode in ["eval", "run"]:
        for max_year, expected in [(2029, "FALSE"), (2099, "TRUE")]:
            assert execute(
                args.cli, mode, "SELECT #2030-01-01#='1/1/30'",
                controls=(
                    "--date-format", "en-us", "--two-digit-year-max", str(max_year)
                ),
            ) == [[expected]]
            count += 1
        assert execute(
            args.cli, mode, "SELECT #2030-01-01#='1/1'",
            controls=("--date-format", "en-us", "--reference-year", "2030"),
        ) == [["TRUE"]]
        assert execute(
            args.cli, mode, "SELECT #1899-12-30 06:00:00#='-0,25'",
            controls=("--date-format", "iso", "--number-format", "de-de"),
        ) == [["TRUE"]]
        count += 2
        execute(args.cli, mode, "SELECT #2030-01-01#='2030-01-01'", success=False)
        count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        original = database.read_bytes()
        tagged = run(
            "java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            RECIPES / "TableDataOracle.java", database, "ScalarSamples",
        )
        moments = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            identity = int(fields[1].removeprefix("I32:"))
            if fields[-1] != "N":
                moments.append((identity, dt.datetime.fromisoformat(fields[-1][1:])))
        for profile in ["iso", "en-us", "de-de"]:
            controls = ("--date-format", profile)
            for identity, moment in moments:
                text = spelling(moment, profile)
                assert execute(
                    args.cli, "run",
                    f"SELECT ID FROM ScalarSamples WHERE Stamp='{text}'",
                    database, controls=controls,
                ) == [[str(identity)]]
                assert execute(
                    args.cli, "run",
                    f"SELECT DCount('*','ScalarSamples',\"Stamp='{text}'\")",
                    database, controls=controls,
                ) == [["1"]]
                count += 2
        assert database.read_bytes() == original
        print(database.name, len(original), hashlib.sha256(original).hexdigest())
    print(
        f"Passed {count} independent Gregorian/serial/native-field predicate controls; "
        "native Access unrun."
    )


if __name__ == "__main__":
    main()
