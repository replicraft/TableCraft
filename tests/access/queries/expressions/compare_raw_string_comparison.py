"""Independent unsigned UTF-16 models for lossless binary StrComp.

Python compares unit tuples without Unicode decoding; native Access is unrun.
"""

import argparse
from pathlib import Path
import random

from compare_cverr import execute


def source(units):
    if not units:
        return "''"
    return "(" + " & ".join(f"ChrW({unit})" for unit in units) + ")"


def sign(left, right):
    return (left > right) - (left < right)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    rng = random.Random(20261024)
    pairs = [
        ([], []), ([], [0]), ([0], []), ([55296], [57343]),
        ([256], [1]), ([55357, 56832], [57344]),
        ([65, 0, 66], [65]), ([65], [65, 32]),
    ]
    pairs += [
        ([rng.randrange(65536) for _ in range(rng.randrange(5))],
         [rng.randrange(65536) for _ in range(rng.randrange(5))])
        for _ in range(128)
    ]
    controls = 0

    for left, right in pairs:
        lhs, rhs = source(left), source(right)
        sql = (
            f"SELECT StrComp({lhs},{rhs},0),StrComp({rhs},{lhs},0),"
            f"StrComp({lhs},{lhs},0),StrComp({lhs},'😀',0)"
        )
        expected = [str(sign(left, right)), str(sign(right, left)), "0",
                    str(sign(left, [55357, 56832]))]
        for mode in ["eval", "run"]:
            assert execute(args.cli, mode, sql) == [expected], (mode, sql)
            controls += len(expected)

    for mode in ["eval", "run"]:
        sql = "SELECT StrComp(ChrB(65),NULL,0),StrComp(NULL,ChrB(65),0)"
        assert execute(args.cli, mode, sql) == [["NULL", "NULL"]]
        controls += 2
        for expression in [
            "StrComp(ChrB(65),ChrB(65),0)",
            "StrComp(ChrB(65),NULL,3)",
            "StrComp(ChrW(55296),'x',1)",
            "StrComp(ChrW(55296),NULL,1/0)",
            "StrComp(ChrW(65536),NULL,0)",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, success=False)
            controls += 1

    print(f"Passed {controls} independent raw UTF-16 StrComp/refusal controls; native Access unrun.")


if __name__ == "__main__":
    main()
