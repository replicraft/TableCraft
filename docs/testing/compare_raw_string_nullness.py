"""Independent NULL-identity controls for lossless String query operands.

Python supplies String versus NULL identity; native Access SQL is not executed.
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
        expressions = []
        expected = []
        for raw in [
            f"ChrW({code})",
            f"ChrB({code % 256})",
            f"LeftB(ChrW({code}),1)",
            f"CStr(LeftB(ChrW({code}),1))",
        ]:
            expressions += [
                f"IsNull({raw})",
                f"{raw} IS NULL",
                f"{raw} IS NOT NULL",
                f"IsNull({raw}+NULL)",
            ]
            expected += ["FALSE", "FALSE", "TRUE", "TRUE"]

        sql = "SELECT " + ",".join(expressions)
        for mode in ["eval", "run"]:
            assert execute(args.cli, mode, sql) == [expected], (mode, sql)
            controls += len(expected)

    for mode in ["eval", "run"]:
        for expression in [
            "IsNull(ChrW(65536))",
            "ChrW(NULL) IS NULL",
            "IsNull(ChrW(55296)+(1/0))",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, success=False)
            controls += 1

    print(f"Passed {controls} independent raw String NULL-identity/refusal controls; native Access unrun.")


if __name__ == "__main__":
    main()
