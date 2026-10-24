"""Original eager IIf controls over lossless String branches.

Python selects byte sequences; only TableCraft executes SQL.
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
    pairs = [(0, 0), (65, 66), (55296, 65), (57343, 255), (55357, 128)]
    pairs += [(rng.randrange(65536), rng.randrange(256)) for _ in range(64)]
    controls = 0

    for unit, byte in pairs:
        raw_yes = unit.to_bytes(2, "little")
        raw_no = bytes([byte])
        for condition, selected in [("True", raw_yes), ("False", raw_no)]:
            call = f"IIf({condition},ChrW({unit}),ChrB({byte}))"
            sql = f"SELECT LenB({call}),VarType({call}),IsNull({call}),AscB({call})"
            expected = [str(len(selected)), "8", "FALSE", str(selected[0])]
            for mode in ["eval", "run"]:
                assert execute(args.cli, mode, sql) == [expected], (mode, sql)
                controls += len(expected)

        sql = f"SELECT IIf(False,ChrW({unit}),'ok'),IIf(NULL,ChrB({byte}),'ok')"
        for mode in ["eval", "run"]:
            assert execute(args.cli, mode, sql) == [["\"ok\"", "\"ok\""]], (mode, sql)
            controls += 2

    for mode in ["eval", "run"]:
        for expression in [
            "LenB(IIf(True,ChrW(55296),1/0))",
            "LenB(IIf(False,ChrW(65536),ChrB(65)))",
            "IIf(True,ChrW(55296),'ok')",
            "LenB(IIf(CVErr(1),ChrW(55296),ChrB(65)))",
            "LenB(IIf('True',ChrW(55296),ChrB(65)))",
            "LenB(IIf(False,1/0,ChrB(65)))",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, success=False)
            controls += 1

    print(f"Passed {controls} independent raw String IIf/selection/refusal controls; native Access unrun.")


if __name__ == "__main__":
    main()
