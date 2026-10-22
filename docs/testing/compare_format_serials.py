"""Independent legacy DATE-to-numeric Format controls and native getter integration.

Original calendar inputs use quarter-day fractions exactly representable in DATE.
Decimal implements the expected fixed-point display. This is not native Access SQL.
"""

import argparse
import datetime as dt
from decimal import Decimal, ROUND_HALF_UP
import hashlib
from pathlib import Path
import random

from compare_browsing import RECIPES, run
from compare_cstr import spelling
from compare_error_messages import execute
from compare_format_dates import JAR_SHA256, literal

EPOCH = dt.date(1899, 12, 30)


def serial(value):
    day = (value.date() - EPOCH).days
    seconds = value.hour * 3600 + value.minute * 60 + value.second
    fraction = Decimal(seconds) / Decimal(86400)
    return Decimal(day) + (fraction if day >= 0 else -fraction)


def fixed(value, profile):
    text = str(value.quantize(Decimal("0.00"), rounding=ROUND_HALF_UP))
    return text.replace(".", ",") if profile == "de-DE" else text


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    values = [
        dt.datetime(100, 1, 1), dt.datetime(1899, 12, 29, 6),
        dt.datetime(1899, 12, 30), dt.datetime(1899, 12, 30, 6),
        dt.datetime(2025, 1, 1, 12), dt.datetime(9999, 12, 31, 18),
    ]
    rng = random.Random(20261021)
    values += [dt.datetime(100, 1, 1) + dt.timedelta(
        days=rng.randrange(3_000_000), hours=rng.randrange(4) * 6
    ) for _ in range(48)]
    count = 0
    for mode in ["eval", "run"]:
        for profile in ["en-US", "de-DE"]:
            for offset in range(0, len(values), 6):
                batch = values[offset:offset + 6]
                sql = "SELECT " + ",".join(
                    f"Format({literal(value)},'0.00')" for value in batch
                )
                expected = [[spelling(fixed(serial(value), profile)) for value in batch]]
                assert execute(args.cli, mode, sql, controls=("--number-format", profile)) == expected
                count += len(batch)
            for sql in [
                "SELECT Format(#2025-01-01#,'0.00')",
                "SELECT Format(#2025-01-01#,'General Number')",
                "SELECT Format(#2025-01-01#,'0.00'),1/0",
            ]:
                controls = () if "1/0" not in sql else ("--number-format", profile)
                execute(args.cli, mode, sql, controls=controls, success=False)
                count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        rows = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            value = None if fields[-1] == "N" else dt.datetime.fromisoformat(fields[-1].removeprefix("Y"))
            rows.append((fields[1].removeprefix("I32:"), value))
        for profile in ["en-US", "de-DE"]:
            expected = [[identity, spelling("" if value is None else fixed(serial(value), profile))]
                        for identity, value in rows]
            assert execute(args.cli, "run",
                           "SELECT ID,Format(Stamp,'0.00') FROM ScalarSamples ORDER BY ID",
                           database, controls=("--number-format", profile)) == expected
            count += len(rows)
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent serial-format/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
