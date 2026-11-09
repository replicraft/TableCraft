"""Independent rounded repetition and UTF-16 search-start controls.

Only TableCraft executes SQL; existing read-only Jackcess ID getters supply native
field references. Source/JAR hashes are checked and no fixtures are changed.
"""

import argparse
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path
import random

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute, quoted


def find_model(source, needle, start, reverse):
    raw = source.encode("utf-16-le")
    target = needle.encode("utf-16-le")
    units = [raw[n:n + 2] for n in range(0, len(raw), 2)]
    match = [target[n:n + 2] for n in range(0, len(target), 2)]
    if reverse and start == -1:
        start = len(units)
    elif start < 1:
        return None
    if not units or start > len(units):
        return 0
    if not match:
        return start
    candidates = [n for n in range(len(units) - len(match) + 1)
                  if units[n:n + len(match)] == match
                  and (n + len(match) <= start if reverse else n >= start - 1)]
    return (candidates[-1] if reverse else candidates[0]) + 1 if candidates else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261021)
    values = [Decimal(text) for text in ["-1.5", "-1.25", "-0.5", "0.5", "1.5", "2.5", "4.5", "8.5"]]
    values += [Decimal(rng.randrange(-8, 48)) / 4 for _ in range(16)]
    count = 0
    for profile in ["en-US", "de-DE"]:
        for number in values:
            text = format(number, "f")
            regional = text.replace(".", ",") if profile == "de-DE" else text
            start = int(number.to_integral_value(rounding=ROUND_HALF_EVEN))
            for operand in [f"CDate({text})", f"'{regional}'"]:
                sql = f"SELECT Len(Space({operand})),String({operand},'65')"
                for mode in ["eval", "run"]:
                    result = execute(args.cli, mode, sql, controls=("--number-format", profile), success=start >= 0)
                    if start >= 0:
                        assert result == [[str(start), quoted("6" * start)]]
                    count += 2
                for reverse in [False, True]:
                    for needle in ["ab", ""]:
                        expected = find_model("é🦀ababa", needle, start, reverse)
                        sql = (f"SELECT InStrRev('é🦀ababa','{needle}',{operand},0)" if reverse
                               else f"SELECT InStr({operand},'é🦀ababa','{needle}',0)")
                        for mode in ["eval", "run"]:
                            result = execute(args.cli, mode, sql, controls=("--number-format", profile), success=expected is not None)
                            if expected is not None:
                                assert result == [[str(expected)]], (sql, result, expected)
                            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = sorted(int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines())
        expected = [[str(n), str(n), quoted("6" * n), str(find_model("ababab", "ab", n, False)), str(find_model("ababab", "ab", n, True))] for n in identities]
        sql = "SELECT ID,Len(Space(CDate(ID))),String(CStr(ID),'65'),InStr(CStr(ID),'ababab','ab',0),InStrRev('ababab','ab',CDate(ID),0) FROM ScalarSamples ORDER BY ID"
        assert execute(args.cli, "run", sql, database, controls=("--number-format", "en-US")) == expected
        count += len(identities) * 4
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent repetition/UTF-16/start/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
