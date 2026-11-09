"""Independent three-valued NULL and issued-Variant predicate controls.

Python supplies NULL truth and type-category expectations; native Access is unrun.
"""

import argparse
import csv
from io import StringIO
from pathlib import Path
import random
import subprocess


def query(cli, mode, sql, expected, options=()):
    result = subprocess.run(
        [str(cli), "query", mode, sql, *options],
        capture_output=True, text=True, encoding="utf-8", check=False,
    )
    if expected is None:
        assert result.returncode == 2 and not result.stdout, (sql, result)
        return 1
    assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
    rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
    assert rows[1:] == expected, (sql, rows, expected)
    return sum(len(row) for row in expected)


def predicates(raw):
    expressions = []
    for operator in ["=", "<>", "<", "<=", ">", ">="]:
        expressions += [f"{raw}{operator}NULL", f"NULL{operator}{raw}"]
    expressions += [
        f"{raw} IN(NULL)", f"{raw} NOT IN(NULL)", f"NULL IN({raw})",
        f"{raw} BETWEEN NULL AND NULL", f"{raw} NOT BETWEEN NULL AND NULL",
    ]
    return expressions


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    rng = random.Random(20261024)
    codes = [0, 65, 55296, 57343] + [rng.randrange(65536) for _ in range(32)]
    controls = 0

    for code in codes:
        for raw in [f"ChrW({code})", f"ChrB({code % 256})", f"CStr(LeftB(ChrW({code}),1))"]:
            expressions = predicates(raw)
            sql = "SELECT " + ",".join(expressions)
            expected = [["NULL"] * len(expressions)]
            for mode in ["eval", "run"]:
                controls += query(args.cli, mode, sql, expected)
            sql = "SELECT " + ",".join(predicates("S")) + f" FROM (SELECT {raw} AS S) AS Q"
            controls += query(args.cli, "run", sql, expected)

            for wildcard in ["ansi89", "ansi92"]:
                options = ["--like-mode", wildcard]
                sql = f"SELECT {raw} LIKE NULL,NULL LIKE {raw},{raw} NOT LIKE NULL,NULL NOT LIKE {raw}"
                controls += query(args.cli, "run", sql, [["NULL"] * 4], options)
                sql = f"SELECT S LIKE NULL,NULL NOT LIKE S FROM (SELECT {raw} AS S) AS Q WHERE IsNull(S LIKE NULL)"
                controls += query(args.cli, "run", sql, [["NULL"] * 2], options)

        source = f" FROM (SELECT ChrW({code}) AS S) AS Q"
        sql = "SELECT S IN(SELECT NULL),S=ANY(SELECT NULL),S=ALL(SELECT NULL),S=ANY(SELECT NULL WHERE False),S=ALL(SELECT NULL WHERE False)" + source
        controls += query(args.cli, "run", sql, [["NULL", "NULL", "NULL", "FALSE", "TRUE"]])
        sql = f"SELECT CVar(ChrW({code}))>CVar(0),CVar(0)<CVar(ChrB({code % 256})),CVar(ChrB({code % 256})) IN(CVar(0),NULL)"
        controls += query(args.cli, "run", sql, [["TRUE", "TRUE", "NULL"]], ["--declared-arithmetic"])

    for mode in ["eval", "run"]:
        for expression in [
            "(ChrW(55296)=NULL) AND (1/0)",
            "ChrW(55296) IN(NULL,1/0)",
            "ChrB(65) BETWEEN NULL AND 1/0",
            "ChrW(65536)=NULL",
            "ChrW(55296)=CVErr(1)",
            "ChrW(55296)='a'",
        ]:
            controls += query(args.cli, mode, "SELECT 1," + expression, None)

    for wildcard in ["ansi89", "ansi92"]:
        for expression in [
            "ChrW(55296) LIKE 1/0", "NULL LIKE ChrW(65536)",
            "ChrW(55296) LIKE '*'", "'*' LIKE ChrB(65)",
        ]:
            controls += query(args.cli, "run", "SELECT 1," + expression, None,
                              ["--like-mode", wildcard])
    controls += query(args.cli, "run", "SELECT ChrB(65) LIKE NULL", None)

    print(f"Passed {controls} independent raw String predicate controls; native Access unrun.")


if __name__ == "__main__":
    main()
