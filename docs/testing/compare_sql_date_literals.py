"""Model SQL date literals with Python calendars and unchanged native DATE getters.

Only TableCraft executes expressions; native Access qualification is unrun.
"""

import argparse
import csv
import datetime
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256

MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
          "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def query(mode, sql, expected=None, options=(), fails=False):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", mode, sql, *map(str, options)],
            capture_output=True, text=True, encoding="utf-8",
        )
        if fails:
            assert result.returncode == 2 and not result.stdout, (sql, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == expected, (sql, rows[1:], expected)
        controls += 1

    rng = random.Random(20261023)
    dates = [
        datetime.datetime(100, 1, 1), datetime.datetime(1899, 12, 29, 6),
        datetime.datetime(2000, 2, 29, 12, 34, 56), datetime.datetime(2026, 1, 2),
        datetime.datetime(9999, 12, 31, 23, 59, 59),
    ]
    for _ in range(43):
        dates.append(datetime.datetime(
            rng.randrange(100, 10_000), rng.randrange(1, 13), rng.randrange(1, 29),
            rng.randrange(24), rng.randrange(60), rng.randrange(60),
        ))

    for mode in ["eval", "run"]:
        for value in dates:
            day = f"{value.month}/{value.day}/{value.year:04}"
            hour = value.hour % 12 or 12
            suffix = "AM" if value.hour < 12 else "PM"
            for literal in [
                f"{day} {value.hour}:{value.minute:02}:{value.second:02}",
                f"{value.day}-{MONTHS[value.month - 1]}-{value.year:04} "
                f"{hour}:{value.minute:02}:{value.second:02}{suffix}",
            ]:
                sql = "SELECT " + ",".join(
                    f"{component}(#{literal}#)"
                    for component in ["Year", "Month", "Day", "Hour", "Minute", "Second"]
                )
                query(mode, sql, [[str(part) for part in [
                    value.year, value.month, value.day, value.hour, value.minute, value.second,
                ]]], options=["--date-format", "de-de"])
        for literal in ["1/2/26", "2/29/1900", "13/1/2026", "Mar 7", "12/31/2024 24:00"]:
            query(mode, f"SELECT 1,#{literal}#", fails=True)
        query(mode, "SELECT #1/2/2026#=#2026-01-02#,CDbl(#12/29/1899 06:00#)", [["TRUE", "-1.25"]])
        query(mode, "SELECT Hour(#12 AM#),Hour(#12 PM#),Minute(#2:24PM#)", [["0", "12", "24"]])

    scalar_controls = controls
    lower = datetime.datetime(2024, 1, 2)
    upper = datetime.datetime(2024, 1, 3, 23, 59, 59)
    for suffix, original in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        before = registered_input_hash(path, original)
        reference = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows",
        ], text=True, encoding="utf-8")
        records = [line.split("\t")[1:] for line in reference.splitlines()]
        identities = [int(row[0][4:]) for row in records]
        assert len(identities) == len(set(identities)) == 20_000
        selected = [
            identity for identity, row in zip(identities, records)
            if row[8] != "N" and lower <= datetime.datetime.fromisoformat(row[8][1:]) <= upper
        ]
        options = [
            "--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
            "--native-steps", "10000000", "--execution-steps", "10000000",
        ]
        for index in [[], ["--index", "IndexedRows", "StampAsc"]]:
            selected_options = [*options, *index]
            query(
                "run", "SELECT Count(*) AS N FROM IndexedRows "
                "WHERE Stamp BETWEEN #1/2/2024# AND #1/3/2024 11:59:59PM#",
                [[str(len(selected))]], selected_options,
            )
            query(
                "run", "SELECT TOP 17 ID FROM IndexedRows "
                "WHERE Stamp BETWEEN #Jan 2, 2024# AND #3-Jan-2024 23:59:59# ORDER BY ID",
                [[str(identity)] for identity in sorted(selected)[:17]], selected_options,
            )
            query(
                "run", "SELECT Count(*) FROM IndexedRows WHERE Stamp=#1/2/24#",
                options=selected_options, fails=True,
            )
        assert hashlib.sha256(path.read_bytes()).hexdigest() == before, "Original changed"
        print(f"{path.name}: unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} SQL date-literal controls ({scalar_controls} scalar); native Access unrun.")


if __name__ == "__main__":
    main()
