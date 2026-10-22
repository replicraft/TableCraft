"""Independent rounded-Double predicate models; no Access SQL oracle executes."""
import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES
from compare_rounded_double import rounded_integer
from compare_union_types import JAR_SHA256


def relations(left, right):
    return [left == right, left != right, left < right, left <= right,
            left > right, left >= right]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def query(mode, sql, expected, options=(), diagnostic=None):
        nonlocal controls
        result = subprocess.run([str(args.cli), "query", mode, sql, *map(str, options)],
                                text=True, encoding="utf-8", capture_output=True)
        if diagnostic:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert diagnostic in result.stderr, (sql, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == expected, (sql, rows, expected)
        controls += 1

    rng = random.Random(20261022)
    values = [-(2**63), -(2**53+3), -(2**53+1), -1, 0, 1,
              2**53-1, 2**53+1, 2**53+3, 2**63-1]
    values += [rng.randrange(-(2**63), 2**63) for _ in range(32)]
    operators = ["=", "<>", "<", "<=", ">", ">="]
    for mode in ["eval", "run"]:
        for value in values:
            left = f"CLngLng({value})"
            rounded = rounded_integer(value)
            for right, target in [(f"CDbl({value})", rounded),
                                  ("CSng(9007199254740992)", float(2**53)),
                                  (f"'{value}'", rounded)]:
                expressions = [f"{left}{op}{right}" for op in operators]
                expressions += [f"{right}{op}{left}" for op in operators]
                expected = relations(rounded, target) + relations(target, rounded)
                query(mode, "SELECT " + ",".join(expressions),
                      [["TRUE" if flag else "FALSE" for flag in expected]],
                      ["--number-format", "en-us"])
        query(mode, "SELECT 9007199254740993=9007199254740992", [["FALSE"]])
        query(mode, "SELECT 9007199254740993 IN (CDbl(9007199254740992),1/0)", None,
              diagnostic="division by zero")
        query(mode, "SELECT 9007199254740993='bad'", None,
              ["--number-format", "en-us"], diagnostic="type mismatch")
        query(mode, "SELECT Null='bad'", [["NULL"]])

    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        output = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows"], text=True, encoding="utf-8")
        tags = [line.split("\t")[1] for line in output.splitlines()]
        assert len(tags) == 20000 and all(tag.startswith("I32:") for tag in tags)
        ids = [int(tag[4:]) for tag in tags]
        options = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
                   "--native-steps", "10000000", "--execution-steps", "10000000"]
        for key in [9007199254740992, 9007199254740996]:
            selected = [value for value in ids if rounded_integer(value+9007199254740992) == float(key)]
            sql = ("SELECT Count(*),Sum(ID) FROM IndexedRows WHERE "
                   f"(ID+9007199254740992)=CDbl({key})")
            expected = [[str(len(selected)), str(sum(selected)) if selected else "NULL"]]
            for index in [None, "IdAsc", "IdDesc"]:
                query("run", sql, expected, [*options, *(["--index", "IndexedRows", index] if index else [])])
        assert path.read_bytes() == before
    print(f"Passed {controls} independent rounded predicate controls; native Access unrun.")


if __name__ == "__main__":
    main()
