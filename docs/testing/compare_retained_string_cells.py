"""Independent unit/byte observations through retained query String cells.

Python supplies integer/unit expectations; native Access SQL is unrun.
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
    codes = [0, 65, 55296, 55357, 56832, 57343]
    codes += [rng.randrange(65536) for _ in range(64)]
    controls = 0

    for code in codes:
        signed = code if code < 32768 else code - 65536
        byte = code % 256
        sql = (
            "SELECT LenB(S),VarType(S),IsNull(S),AscW(S),AscB(B) "
            f"FROM (SELECT ChrW({code}) AS S,LeftB(ChrW({code}),1) AS B) AS Q"
        )
        assert execute(args.cli, "run", sql) == [["2", "8", "FALSE", str(signed), str(byte)]]
        controls += 5

        source = f"(SELECT ChrB({byte}) AS S UNION ALL SELECT ChrW({code})) AS Q"
        assert execute(args.cli, "run", f"SELECT LenB(S),AscB(S) FROM {source}") == [
            ["1", str(byte)], ["2", str(byte)]
        ]
        controls += 4
        assert execute(args.cli, "run", f"SELECT LenB(First(S)),LenB(Last(S)) FROM {source}") == [["1", "2"]]
        controls += 2

        source = f"(SELECT ChrW({code}) AS S) AS Q"
        assert execute(args.cli, "run", f"SELECT (SELECT AscW(Q.S)) FROM {source}") == [[str(signed)]]
        assert execute(args.cli, "run", f"SELECT StrComp(S,ChrW({code}),0) FROM {source}") == [["0"]]
        assert execute(args.cli, "run", f"SELECT IsNull(S+NULL),Len(S&'x') FROM {source}") == [["TRUE", "2"]]
        controls += 4

    for sql in [
        "SELECT 1,S FROM (SELECT ChrB(65) AS S) AS Q",
        "SELECT 1,S FROM (SELECT ChrW(55296) AS S) AS Q",
        "SELECT LenB(S) FROM (SELECT ChrW(65536) AS S) AS Q",
    ]:
        execute(args.cli, "run", sql, success=False)
        controls += 1

    print(f"Passed {controls} independent retained String cell controls; native Access unrun.")


if __name__ == "__main__":
    main()
