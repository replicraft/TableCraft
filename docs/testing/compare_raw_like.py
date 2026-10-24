"""Independent raw UTF-16 LIKE counts and exact byte-slice controls.

Python supplies unit counts, slice bytes and NULL truth; General weight semantics
remain the existing provisional provider. No native Access execution is claimed.
"""

import argparse
from pathlib import Path
import random

from compare_raw_string_predicates import query


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    rng = random.Random(20261024)
    codes = [0, 32, 65, 223, 55296, 56319, 56320, 57343]
    codes += [rng.randrange(55296, 57344) for _ in range(32)]
    codes += [rng.randrange(65536) for _ in range(32)]
    controls = 0

    for collation in ["general-legacy", "general-2010"]:
        for mode, one, many in [("ansi89", "?", "*"), ("ansi92", "_", "%")]:
            options = ["--collation", collation, "--like-mode", mode]
            for code in codes:
                sql = (
                    f"SELECT S LIKE '{one}',S LIKE '{one}{one}',S LIKE '{many}',"
                    f"S LIKE ChrW({code}),S LIKE '[]',S NOT LIKE '{one}',"
                    f"NULL LIKE S,S LIKE '['&ChrW({code})&']' "
                    f"FROM (SELECT ChrW({code}) AS S) AS Q"
                )
                controls += query(args.cli, "run", sql, [[
                    "TRUE", "FALSE", "TRUE", "TRUE", "FALSE", "FALSE", "NULL", "TRUE",
                ]], options)
                sql = (
                    f"SELECT LenB(S) FROM (SELECT ChrW({code}) AS S) AS Q "
                    f"WHERE S LIKE '{one}'"
                )
                controls += query(args.cli, "run", sql, [["2"]], options)

            for _ in range(32):
                left, right = rng.randrange(65536), rng.randrange(65536)
                raw = left.to_bytes(2, "little") + right.to_bytes(2, "little")
                expected = int.from_bytes(raw[1:3], "little")
                sql = (
                    f"SELECT MidB(ChrW({left})&ChrW({right}),2,2) "
                    f"LIKE ChrW({expected})"
                )
                controls += query(args.cli, "run", sql, [["TRUE"]], options)

            for sql in [
                "SELECT ChrW(55296)&'ss' LIKE ChrW(55296)&'ß'",
                "SELECT ChrW(55296)&'ss' LIKE ChrW(55296)&'[ß]'",
            ]:
                controls += query(args.cli, "run", sql, [["TRUE"]], options)

            for expression in [
                f"ChrB(65) LIKE '{many}'", f"'{many}' LIKE ChrB(65)",
                "ChrW(55296) LIKE '['", "ChrW(55296) LIKE 1/0",
            ]:
                controls += query(args.cli, "run", "SELECT 1," + expression, None, options)

    controls += query(args.cli, "run", "SELECT ChrW(55296) LIKE '?'", None,
                      ["--collation", "ordinal", "--like-mode", "ansi89"])
    print(f"Passed {controls} independent raw LIKE controls; native Access unrun.")


if __name__ == "__main__":
    main()
