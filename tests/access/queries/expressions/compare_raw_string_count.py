"""Original NULL/cardinality controls for Count over raw String expressions.

Python models row membership only; no Microsoft Access SQL is executed.
"""

import argparse
from pathlib import Path
import random

from compare_cverr import execute


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    rng = random.Random(20261024)
    cases = [[], [0], [65], [55296], [57343], [55357, 56832]]
    cases += [
        [rng.randrange(65536) for _ in range(rng.randrange(1, 9))]
        for _ in range(64)
    ]
    controls = 0

    for values in cases:
        source = " UNION ALL ".join(f"SELECT {value} AS N" for value in values)
        if not values:
            source = "SELECT 0 AS N WHERE False"

        expressions = [
            "ChrW(N)",
            "CVar(ChrW(N))",
            "CStr(ChrW(N))",
            "ChrB(N Mod 256)",
            "LeftB(ChrW(N),1)",
            "CStr(LeftB(ChrW(N),1))",
            "ChrW(N)+NULL",
            "NULL",
            "*",
        ]
        projection = ",".join(f"Count({expression})" for expression in expressions)
        sql = f"SELECT {projection} FROM ({source}) AS Q"
        expected = [str(len(values))] * 6 + ["0", "0", str(len(values))]
        assert execute(args.cli, "run", sql) == [expected], sql
        controls += len(expected)

        selected = [value for value in values if value >= 55296]
        sql = f"SELECT Count(ChrW(N)),Count(LeftB(ChrW(N),1)) FROM ({source}) AS Q WHERE N>=55296"
        assert execute(args.cli, "run", sql) == [[str(len(selected))] * 2], sql
        controls += 2

    for expression in ["ChrW(65536)", "ChrW(NULL)", "CStr(NULL)", "ChrW(55296)+(1/0)"]:
        execute(args.cli, "run", f"SELECT Count({expression})", success=False)
        controls += 1

    print(f"Passed {controls} independent raw String Count/cardinality/refusal controls; native Access unrun.")


if __name__ == "__main__":
    main()
