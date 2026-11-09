"""Independent per-bit logical controls and inert native integer-field queries.

No Access SQL engine is invoked. Original native sources are hash checked before
and after queries; Jackcess supplies only read-only typed integer reference data.
"""

import argparse
import hashlib
from pathlib import Path
import random

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute


def bits(a, b, width, truth):
    output = 0
    for position in range(width):
        left = (a >> position) & 1
        right = (b >> position) & 1
        output += int(truth[left * 2 + right]) * 2 ** position
    return output


def signed(value, width):
    return value - 2 ** width if value >= 2 ** (width - 1) else value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    operators = [
        ("And", [0, 0, 0, 1]),
        ("Or", [0, 1, 1, 1]),
        ("Xor", [0, 1, 1, 0]),
        ("Eqv", [1, 0, 0, 1]),
        ("Imp", [1, 1, 0, 1]),
    ]
    rng = random.Random(20261021)
    count = 0
    for width, convert in [(8, "CByte"), (16, "CInt"), (32, "CLng"), (64, "CLngLng")]:
        pairs = [(0, 0), (1, 2), (2 ** (width - 1), 1), (2 ** width - 1, 0)]
        pairs += [(rng.randrange(2 ** width), rng.randrange(2 ** width)) for _ in range(24)]
        for a, b in pairs:
            left = a if width == 8 else signed(a, width)
            right = b if width == 8 else signed(b, width)
            expressions = [f"{convert}({left}) {op} {convert}({right})" for op, _ in operators]
            expressions.append(f"Not {convert}({left})")
            results = [bits(a, b, width, truth) for _, truth in operators]
            results.append(2 ** width - 1 - a)
            expected = [[str(n if width == 8 else signed(n, width)) for n in results]]
            for mode in ["eval", "run"]:
                assert execute(args.cli, mode, "SELECT " + ",".join(expressions)) == expected
                count += len(results)
    for predicate, selected in [("1 And 2", False), ("Not 1", True), ("Not -1", False), ("0.25", True)]:
        expected = [["99"]] if selected else []
        assert execute(args.cli, "run", f"SELECT 99 WHERE {predicate}") == expected
        count += 1
    for mode in ["eval", "run"]:
        for sql in ["SELECT 0 And (1/0)", "SELECT True Or (1/0)", "SELECT Not 1e40"]:
            execute(args.cli, mode, sql, success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = sorted(int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines())
        sql = "SELECT ID,ID And 2,Not ID FROM ScalarSamples ORDER BY ID"
        expected = [[str(n), str(bits(n, 2, 32, operators[0][1])), str(-n - 1)] for n in identities]
        assert execute(args.cli, "run", sql, database) == expected
        sql = "SELECT ID FROM ScalarSamples WHERE ID And 2 ORDER BY ID"
        assert execute(args.cli, "run", sql, database) == [[str(n)] for n in identities if bits(n, 2, 32, operators[0][1])]
        count += len(identities) * 3 + 1
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent integer/predicate/eager/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
