"""Check non-NULL expression counting with independent ID/state models.

Optional native controls read immutable registered large-index inputs through
Jackcess getters. No Access SQL or active database content is executed.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--directory", type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.directory):
        parser.error("--jar and --directory must be supplied together")
    controls = 0

    def check(sql, expected, options=()):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", "run", sql, *map(str, options)],
            text=True, encoding="utf-8", capture_output=True,
        )
        if expected is None:
            assert result.returncode == 2 and not result.stdout, (sql, result)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == [[str(value) for value in row] for row in expected], (sql, rows)
        controls += 1

    rng = random.Random(20261022)
    for _ in range(32):
        ids = [rng.randrange(65536) for _ in range(rng.randrange(1, 25))]
        source = "(" + " UNION ALL ".join(f"SELECT {value} AS ID" for value in ids) + ") AS Q"
        non_null = sum(value % 3 != 0 for value in ids)
        check(
            "SELECT Count(CVErr(ID)),Count(IIf(ID Mod 3=0,Null,CVErr(ID))),Count(*) FROM " + source,
            [[len(ids), non_null, len(ids)]],
        )
        check(
            "SELECT Count(Problem),Count(*) FROM "
            "(SELECT IIf(ID Mod 3=0,Null,CVErr(ID)) AS Problem FROM " + source + ") AS R",
            [[non_null, len(ids)]],
        )
    for sql in [
        "SELECT Count(CVErr(0)),Count(CVErr(65535))",
        "SELECT Count(CVErr(-1)) FROM (SELECT 1 AS ID) AS Q WHERE ID=0",
    ]:
        check(sql, [[1, 1]] if "65535" in sql else [[0]])
    for sql in [
        "SELECT 1,Count(CVErr(-1))",
        "SELECT 1,Count(IIf(True,CVErr(0),1/0))",
        "SELECT 1,Sum(CVErr(2001))",
        "SELECT 1,Avg(CVErr(2001))",
    ]:
        check(sql, None)

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
        for suffix, digest in INPUT_HASHES.items():
            path = args.directory / ("index-" + suffix)
            digest = registered_input_hash(path, digest)
            before = path.read_bytes()
            assert hashlib.sha256(before).hexdigest() == digest
            output = subprocess.check_output(
                ["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                 str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows"],
                text=True, encoding="utf-8",
            )
            tags = [line.split("\t")[1] for line in output.splitlines()]
            assert all(tag.startswith("I32:") for tag in tags)
            ids = [int(tag.removeprefix("I32:")) for tag in tags]
            assert len(ids) == 20000
            budgets = [
                "--database", path,
                "--native-bytes", "67108864",
                "--native-rows", "100000",
                "--native-steps", "10000000",
                "--execution-steps", "10000000",
            ]
            expected = [[len(ids), sum(value % 3 != 0 for value in ids), len(ids)]]
            sql = (
                "SELECT Count(CVErr(ID)),Count(IIf(ID Mod 3=0,Null,CVErr(ID))),"
                "Count(*) FROM IndexedRows"
            )
            for index in [None, "IdAsc", "IdDesc"]:
                options = budgets if index is None else [*budgets, "--index", "IndexedRows", index]
                check(sql, expected, options)
            check(
                "SELECT DCount('CVErr(ID)','IndexedRows'),"
                "DCount('CVErr(ID)','IndexedRows','ID<100')",
                [[len(ids), sum(value < 100 for value in ids)]], budgets,
            )
            for index in ["IdAsc", "IdDesc"]:
                check(
                    "SELECT Count(CVErr(ID)) FROM IndexedRows WHERE ID>=10 AND ID<=20",
                    [[sum(10 <= value <= 20 for value in ids)]],
                    [*budgets, "--index", "IndexedRows", index],
                )
            check(
                "SELECT Count(IIf(ID=20000,1/0,CVErr(ID))) FROM IndexedRows",
                None, budgets,
            )
            assert path.read_bytes() == before
    print(f"Passed {controls} independent Error/NULL/count/native controls; native Access unrun.")


if __name__ == "__main__":
    main()
