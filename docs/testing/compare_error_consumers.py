"""Model explicit Error extrema and concatenation/numeric-aggregate admission."""

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


def error_cell(code):
    return "NULL" if code is None else f"#ERROR({code})"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--directory", type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.directory):
        parser.error("--jar and --directory must be supplied together")
    controls = 0

    def check(sql, expected=None, diagnostic=None, options=(), mode="run"):
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
    for _ in range(64):
        ids = [rng.randrange(65536) for _ in range(rng.randrange(1, 25))]
        source = "(" + " UNION ALL ".join(f"SELECT {code} AS ID" for code in ids) + ") AS Q"
        present = [code for code in ids if code % 3 != 0]
        low = min(present, default=None)
        high = max(present, default=None)
        expression = "IIf(ID Mod 3=0,Null,CVErr(ID))"
        check(f"SELECT Min({expression}),Max({expression}) FROM {source}",
              [[error_cell(low), error_cell(high)]])
        check(f"SELECT Min(Problem),Max(Problem) FROM "
              f"(SELECT {expression} AS Problem FROM {source}) AS R",
              [[error_cell(low), error_cell(high)]])
    for mode in ["eval", "run"]:
        for expression in ["CVErr(0)&''", "''&CVErr(65535)", "CVErr(0)&Null",
                           "Null&CVErr(0)", "CVErr(0)&CVErr(1)"]:
            check("SELECT 1," + expression, diagnostic="expression type mismatch", mode=mode)
        for expression in ["CVErr(0)&(1/0)", "(1/0)&CVErr(0)"]:
            check("SELECT 1," + expression, diagnostic="division by zero", mode=mode)
        check("SELECT CStr(CVErr(2001))&'!'", [['"Error 2001!"']], mode=mode)
    for function in ["Sum", "Avg", "Var", "VarP", "StDev", "StDevP"]:
        check(f"SELECT 1,{function}(CVErr(0))", diagnostic="expression type mismatch")
        check(f"SELECT 1,{function}(IIf(True,CVErr(0),1/0))", diagnostic="division by zero")
    check("SELECT Min(CVErr(0)),Max(CVErr(65535))", [[error_cell(0), error_cell(65535)]])
    check("SELECT Min(CVErr(-1)),Max(CVErr(-1)) FROM (SELECT 1 AS ID) AS Q WHERE ID=0",
          [["NULL", "NULL"]])

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
            assert len(tags) == 20000 and all(tag.startswith("I32:") for tag in tags)
            ids = [int(tag.removeprefix("I32:")) for tag in tags]
            assert all(0 <= code <= 65535 for code in ids)
            budgets = [
                "--database", path,
                "--native-bytes", "67108864",
                "--native-rows", "100000",
                "--native-steps", "10000000",
                "--execution-steps", "10000000",
            ]
            low, high = min(ids), max(ids)
            present = [code for code in ids if code % 5 != 0]
            for index in [None, "IdAsc", "IdDesc"]:
                options = budgets if index is None else [*budgets, "--index", "IndexedRows", index]
                for sql in [
                    "SELECT Min(CVErr(ID)),Max(CVErr(ID)) FROM IndexedRows",
                    "SELECT Min(Problem),Max(Problem) FROM "
                    "(SELECT CVErr(ID) AS Problem FROM IndexedRows) AS Q",
                    "SELECT DMin('CVErr(ID)','IndexedRows'),DMax('CVErr(ID)','IndexedRows')",
                ]:
                    check(sql, [[error_cell(low), error_cell(high)]], options=options)
                check("SELECT CStr(Min(CVErr(ID))),CLng(Max(CVErr(ID))) FROM IndexedRows",
                      [[f'"Error {low}"', str(high)]], options=options)
                check("SELECT Min(IIf(ID Mod 5=0,Null,CVErr(ID))),"
                      "Max(IIf(ID Mod 5=0,Null,CVErr(ID))) FROM IndexedRows",
                      [[error_cell(min(present)), error_cell(max(present))]], options=options)
                for function in ["Sum", "Avg", "Var", "VarP", "StDev", "StDevP"]:
                    check(f"SELECT 1,{function}(CVErr(ID)) FROM IndexedRows",
                          diagnostic="expression type mismatch", options=options)
                for function in ["DSum", "DAvg", "DVar", "DVarP", "DStDev", "DStDevP"]:
                    check(f"SELECT 1,{function}('CVErr(ID)','IndexedRows')",
                          diagnostic="expression type mismatch", options=options)
                for function in ["DMin", "DMax"]:
                    check(f"SELECT 1,{function}('IIf(ID=1,CVErr(ID),ID)','IndexedRows')",
                          diagnostic="expression type mismatch", options=options)
                check("SELECT 1,Min(IIf(ID=20000,1/0,CVErr(ID))) FROM IndexedRows",
                      diagnostic="division by zero", options=options)
                check("SELECT 1,CVErr(ID)&'x' FROM IndexedRows WHERE ID=10",
                      diagnostic="expression type mismatch", options=options)
            assert path.read_bytes() == before
    print(f"Passed {controls} independent Error aggregate/concatenation/native controls; native Access unrun.")


if __name__ == "__main__":
    main()
