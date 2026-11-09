"""Independent rounded Replace start/count and UTF-16 suffix controls.

Decimal and Python replacement provide separate expectations. Only TableCraft
executes SQL, with read-only native ID getters and unchanged source hashes.
"""

import argparse
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path
import random

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute, quoted


def model(source, find, replacement, start, count):
    start = int(start.to_integral_value(rounding=ROUND_HALF_EVEN))
    count = int(count.to_integral_value(rounding=ROUND_HALF_EVEN))
    if start < 1 or count < -1:
        return None
    raw = source.encode("utf-16-le")
    try:
        suffix = raw[(start - 1) * 2:].decode("utf-16-le", errors="strict")
    except UnicodeDecodeError:
        return None
    return suffix if not find or count == 0 else suffix.replace(find, replacement, count)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261021)
    pairs = [(Decimal(start), Decimal(count)) for start, count in [
        ("0.5", "2.5"), ("1.5", "2.5"), ("2.5", "-1.25"),
        ("3.5", "-0.5"), ("8.5", "1.5"), ("2.5", "-1.5"),
    ]]
    pairs += [(Decimal(rng.randrange(-4, 36)) / 4, Decimal(rng.randrange(-8, 16)) / 4) for _ in range(18)]
    count_controls = 0
    for profile in ["en-US", "de-DE"]:
        for start, count in pairs:
            start_text, count_text = format(start, "f"), format(count, "f")
            if profile == "de-DE":
                start_text, count_text = start_text.replace(".", ","), count_text.replace(".", ",")
            positions = [(f"CDate({start})", f"CDate({count})"), (f"'{start_text}'", f"'{count_text}'")]
            for source, find, replacement in [("banana", "an", "X"), ("aaaa", "a", "aa"), ("é🦀ababa", "ab", "X"), ("banana", "", "X")]:
                expected = model(source, find, replacement, start, count)
                for first, limit in positions:
                    sql = f"SELECT Replace('{source}','{find}','{replacement}',{first},{limit},0)"
                    for mode in ["eval", "run"]:
                        result = execute(args.cli, mode, sql, controls=("--number-format", profile), success=expected is not None)
                        if expected is not None:
                            assert result == [[quoted(expected)]], (sql, result, expected)
                        count_controls += 1
        for sql in ["SELECT Replace(NULL,'a','b','bad',1/0)", "SELECT Replace('','a','b','0','0')", "SELECT Replace('abc','a','b','2147483647.5','0')"]:
            if profile == "de-DE":
                sql = sql.replace("2147483647.5", "2147483647,5")
            execute(args.cli, "run", sql, controls=("--number-format", profile), success=False)
            count_controls += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = sorted(int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines())
        expected = [[str(n), quoted(model("banana", "an", "X", Decimal(n), Decimal(n) + Decimal("0.5"))), quoted(model("aaaa", "a", "aa", Decimal(n), Decimal("-1.25")))] for n in identities]
        sql = "SELECT ID,Replace('banana','an','X',CDate(ID),CStr(ID)&'.5'),Replace('aaaa','a','aa',CStr(ID),CDate(-1.25)) FROM ScalarSamples ORDER BY ID"
        assert execute(args.cli, "run", sql, database, controls=("--number-format", "en-US")) == expected
        count_controls += len(identities) * 2
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count_controls} independent Replace/rounding/UTF-16/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
