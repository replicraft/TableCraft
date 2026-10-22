"""Check standard Error comparisons and logical rejection against Python models."""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES
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

    def check(mode, sql, expected=None, diagnostic=None, options=()):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", mode, sql, *map(str, options)],
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

    rng = random.Random(20261022)
    pairs = [(0, 0), (0, 65535), (65535, 0), (65535, 65535)]
    pairs += [(rng.randrange(65536), rng.randrange(65536)) for _ in range(64)]
    for mode in ["eval", "run"]:
        for left, right in pairs:
            comparisons = [left == right, left != right, left < right,
                           left <= right, left > right, left >= right]
            expressions = [
                f"CVErr({left}) {operator} CVErr({right})"
                for operator in ["=", "<>", "<", "<=", ">", ">="]
            ]
            check(mode, "SELECT " + ",".join(expressions),
                  [["TRUE" if value else "FALSE" for value in comparisons]])
            for low, high in [(0, 65535), (right, left), (right, right)]:
                inside = min(low, high) <= left <= max(low, high)
                check(mode, f"SELECT CVErr({left}) BETWEEN CVErr({low}) AND CVErr({high})",
                      [["TRUE" if inside else "FALSE"]])
            check(mode, f"SELECT CVErr({left}) IN (CVErr({right}),CVErr(0))",
                  [["TRUE" if left in [right, 0] else "FALSE"]])
        for operator in ["=", "<>", "<", "<=", ">", ">=", "And", "Or", "Xor", "Eqv", "Imp"]:
            for left, right in [("CVErr(0)", "Null"), ("1", "CVErr(0)")]:
                check(mode, f"SELECT 1,({left}) {operator} ({right})",
                      diagnostic="expression type mismatch")
            check(mode, f"SELECT 1,CVErr(0) {operator} (1/0)", diagnostic="division by zero")
        for sql, diagnostic in [
            ("SELECT Not CVErr(0)", "expression type mismatch"),
            ("SELECT CVErr(0) IN (CVErr(0),Null)", "expression type mismatch"),
            ("SELECT CVErr(0) IN (CVErr(0),1/0)", "division by zero"),
            ("SELECT CVErr(0) BETWEEN CVErr(0) AND (1/0)", "division by zero"),
        ]:
            check(mode, sql, diagnostic=diagnostic)

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
        for suffix, digest in INPUT_HASHES.items():
            path = args.directory / ("index-" + suffix)
            before = path.read_bytes()
            assert hashlib.sha256(before).hexdigest() == digest
            output = subprocess.check_output(
                ["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                 str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows"],
                text=True, encoding="utf-8",
            )
            tags = [line.split("\t")[1] for line in output.splitlines()]
            assert len(tags) == 20000 and all(tag.startswith("I32:") for tag in tags)
            ids = [int(tag.removeprefix("I32:")) for tag in tags]
            budgets = [
                "--database", path,
                "--native-bytes", "67108864",
                "--native-rows", "100000",
                "--native-steps", "10000000",
                "--execution-steps", "10000000",
            ]
            expected = [[str(sum(value >= 10000 for value in ids))]]
            for index in [None, "IdAsc", "IdDesc"]:
                options = budgets if index is None else [*budgets, "--index", "IndexedRows", index]
                for sql in [
                    "SELECT Count(*) FROM IndexedRows WHERE CVErr(ID)>=CVErr(10000)",
                    "SELECT Count(*) FROM (SELECT CVErr(ID) AS Problem FROM IndexedRows) AS Q WHERE Problem>=CVErr(10000)",
                    "SELECT DCount('*','IndexedRows','CVErr(ID)>=CVErr(10000)')",
                ]:
                    check("run", sql, expected, options=options)
                for sql in [
                    "SELECT CVErr(10000) IN (SELECT CVErr(ID) FROM IndexedRows WHERE ID>=9999 AND ID<=10001)",
                    "SELECT CVErr(10000)=ANY(SELECT CVErr(ID) FROM IndexedRows WHERE ID>=9999 AND ID<=10001)",
                    "SELECT CVErr(9999)<=ALL(SELECT CVErr(ID) FROM IndexedRows WHERE ID>=9999 AND ID<=10001)",
                ]:
                    check("run", sql, [["TRUE"]], options=options)
                check("run", "SELECT CVErr(ID)=(1/(ID-ID)) FROM IndexedRows WHERE ID=10",
                      diagnostic="division by zero", options=options)
            assert path.read_bytes() == before
    print(f"Passed {controls} independent Error predicate/model/native controls; native Access unrun.")


if __name__ == "__main__":
    main()
