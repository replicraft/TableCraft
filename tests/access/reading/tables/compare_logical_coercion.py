"""Independent nearest-even, per-bit and inert native-field logical controls.

Only TableCraft executes SQL. Decimal and Python calendar values supply separate
expectations, with unchanged native source hashes and read-only Jackcess getters.
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
from compare_integer_logic import bits, signed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261021)
    values = [Decimal(text) for text in ["-3.5", "-2.5", "-1.5", "-0.5", "-0.25", "-0", "0.25", "0.5", "1.5", "2.5", "3.5"]]
    values += [Decimal(rng.randrange(-4000, 4000)) / 4 for _ in range(24)]
    count = 0
    for profile in ["en-US", "de-DE"]:
        for number in values:
            text = format(number, "f")
            regional = text.replace(".", ",") if profile == "de-DE" else text
            rounded = int(number.to_integral_value(rounding=ROUND_HALF_EVEN))
            # Eighth/quarter/half values in this corpus are exactly binary32/64
            # representable; no decimal-to-IEEE equivalence is assumed elsewhere.
            inputs = [f"CSng({text})", f"CDbl({text})", f"CCur('{regional}')", f"CDec('{regional}')", f"CDate({text})", f"'{regional}'"]
            sql = "SELECT " + ",".join(f"Not {value},{value} And CByte(3),{value} Or CLngLng(9007199254740993)" for value in inputs)
            expected = []
            for _ in inputs:
                expected += [str(-rounded - 1), str(bits(rounded, 3, 32, [0, 0, 0, 1])), str(signed(bits(rounded, 9007199254740993, 64, [0, 1, 1, 1]), 64))]
            for mode in ["eval", "run"]:
                assert execute(args.cli, mode, sql, controls=("--number-format", profile)) == [expected]
                count += len(expected)
            expected = [["99"]] if number else []
            assert execute(args.cli, "run", f"SELECT 99 WHERE '{regional}'", controls=("--number-format", profile)) == expected
            count += 1
        for sql in ["SELECT Not 'bad'", "SELECT 'bad' And (1/0)", "SELECT CByte(0) And CDec(2147483647.5)", "SELECT Not CVErr(0)"]:
            for mode in ["eval", "run"]:
                execute(args.cli, mode, sql, controls=("--number-format", profile), success=False)
                count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        expected = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            identity = int(fields[1].removeprefix("I32:"))
            if fields[-1] == "N":
                complement = "NULL"
            else:
                stamp = dt.datetime.fromisoformat(fields[-1].removeprefix("Y"))
                days = (stamp.date() - dt.date(1899, 12, 30)).days
                fraction = Decimal(stamp.hour * 3600 + stamp.minute * 60 + stamp.second) / 86400
                serial = Decimal(days) + (fraction if days >= 0 else -fraction)
                complement = str(-int(serial.to_integral_value(rounding=ROUND_HALF_EVEN)) - 1)
            expected.append([str(identity), complement, str(identity % 2), str(-identity - 1)])
        sql = "SELECT ID,Not Stamp,CCur(ID) And CInt(1),Not CStr(ID) FROM ScalarSamples ORDER BY ID"
        assert execute(args.cli, "run", sql, database, controls=("--number-format", "en-US")) == expected
        sql = "SELECT ID FROM ScalarSamples WHERE CStr(ID) ORDER BY ID"
        assert execute(args.cli, "run", sql, database, controls=("--number-format", "en-US")) == [[row[0]] for row in expected]
        count += 4 * len(expected) + 1
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent rounding/logical/Text/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
