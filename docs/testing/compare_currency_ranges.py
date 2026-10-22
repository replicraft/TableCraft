"""Check typed Currency range hints against independent stored scaled values."""

import argparse
import csv
from decimal import Decimal
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES
from compare_union_types import JAR_SHA256

MONEY_HASHES = {
    "2016": "2581de772a345fa3185bf546ae53e0607fd42abf6595b87725cd3d61299da414",
    "2019": "444ce6b3223943b7d1729b22de4278f7bb5b0f6e0468366e8ffc706754113cba",
}


def money(value):
    return format(Decimal(value).scaleb(-4), "f")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory", "currency2016", "currency2019"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    inputs = [
        (args.directory / ("index-" + suffix), digest, "IndexedRows", "Money", 5, ["MoneyAsc"])
        for suffix, digest in INPUT_HASHES.items()
    ]
    inputs += [
        (getattr(args, "currency" + profile), digest, "MoneyIndexed", "Amount", 1, ["MoneyAsc", "MoneyDesc"])
        for profile, digest in MONEY_HASHES.items()
    ]
    controls = 0

    def check(sql, options, expected=None, diagnostic=None):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", "run", sql, *map(str, options)],
            text=True, encoding="utf-8", capture_output=True,
        )
        if diagnostic:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert diagnostic in result.stderr, (sql, diagnostic, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == expected, (sql, rows, expected)
        controls += 1

    for path, digest, table, column, position, indexes in inputs:
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        output = subprocess.check_output(
            ["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
             str(RECIPES / "TableDataOracle.java"), str(path), table],
            text=True, encoding="utf-8",
        )
        rows = []
        for line in output.splitlines():
            fields = line.split("\t")[1:]
            assert fields[0].startswith("I32:")
            tag = fields[position]
            assert tag == "N" or tag.startswith("C")
            rows.append((int(fields[0][4:]), None if tag == "N" else int(tag[1:])))
        budgets = [
            "--database", path, "--native-bytes", "67108864",
            "--native-rows", "100000", "--native-steps", "10000000",
            "--execution-steps", "10000000",
        ]
        for index in indexes:
            options = [*budgets, "--index", table, index]
            for value in [-(2**63), -9007199254740993, -1, 0, 1, 9007199254740993, 2**63-1]:
                ids = [identifier for identifier, key in rows if key == value]
                expected = [[str(len(ids)), str(sum(ids)) if ids else "NULL"]]
                sql = f"PARAMETERS P CURRENCY; SELECT Count(*) AS N,Sum(ID) AS S FROM {table} WHERE [{column}]=P"
                check(sql, [*options, "--param", f"P=currency:{money(value)}"], expected)
            for lower, upper in [(-1, 1), (0, 0), (1, -1), (-(2**63), 2**63-1)]:
                ids = [identifier for identifier, key in rows if key is not None and lower <= key < upper]
                sql = f"PARAMETERS L CURRENCY,U CURRENCY; SELECT Count(*) AS N,Sum(ID) AS S FROM {table} WHERE L<=[{column}] AND [{column}]<U"
                check(sql, [*options, "--param", f"L=currency:{money(lower)}", "--param", f"U=currency:{money(upper)}"],
                      [[str(len(ids)), str(sum(ids)) if ids else "NULL"]])
            for predicate, accepts in [
                (f"[{column}]=CCur(0.0001)", lambda value: value == 1),
                (f"[{column}]>=CCur(-0.0001) AND [{column}]<CCur(0.0001)", lambda value: value is not None and -1 <= value < 1),
                (f"[{column}] IS NULL", lambda value: value is None),
                (f"[{column}] IS NOT NULL", lambda value: value is not None),
            ]:
                count = sum(accepts(value) for _, value in rows)
                check(f"SELECT Count(*) AS N FROM {table} WHERE {predicate}", options, [[str(count)]])
            check(f"SELECT Count(*) FROM {table} WHERE [{column}]>=CCur(0) AND 1/(ID-ID)=0",
                  options, diagnostic="division by zero")
            if len(rows) == 20000:
                small = list(options)
                small[small.index("--native-rows") + 1] = "6000"
                count = sum(value == 0 for _, value in rows)
                check(f"SELECT Count(*) FROM {table} WHERE [{column}]=CCur(0)", small, [[str(count)]])
                check(f"SELECT (SELECT Count(*) FROM {table} WHERE [{column}]=CCur(0)),(SELECT Count(*) FROM {table})",
                      small, diagnostic="structural limit")
        assert path.read_bytes() == before
    print(f"Passed {controls} independent Currency range/parameter/cache/native controls; native Access unrun.")


if __name__ == "__main__":
    main()
