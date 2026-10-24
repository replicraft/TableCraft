"""Original byte-selection controls for query Nz, Switch and Choose.

Python supplies independent choice/nullness expectations; native Access is unrun.
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
        wide = f"ChrW({unit})"
        narrow = f"ChrB({byte})"
        expressions = [
            f"LenB(Choose(1,{wide},{narrow}))",
            f"LenB(Choose(2,{wide},{narrow}))",
            f"LenB(Switch(False,{narrow},True,{wide}))",
            f"LenB(Nz({wide},{narrow}))",
            f"LenB(Nz(NULL,{narrow}))",
            f"IsNull(Switch(False,{wide}))",
            f"IsNull(Choose(0,{wide},{narrow}))",
            "IsNull(Nz(NULL))",
            f"IsNull(Switch(True,NULL,True,{wide}))",
            f"VarType(Choose(2,{wide},{narrow}))",
        ]
        expected = ["2", "1", "2", "2", "1", "TRUE", "TRUE", "FALSE", "TRUE", "8"]
        sql = "SELECT " + ",".join(expressions)
        for mode in ["eval", "run"]:
            assert execute(args.cli, mode, sql) == [expected], (mode, sql)
            controls += len(expected)

        sql = f"SELECT Nz('ok',{wide}),Switch(False,{wide},True,'ok'),Choose(2,{narrow},'ok')"
        for mode in ["eval", "run"]:
            assert execute(args.cli, mode, sql) == [['"ok"'] * 3], (mode, sql)
            controls += 3

    for mode in ["eval", "run"]:
        for expression in [
            "LenB(Nz(ChrW(55296),1/0))",
            "LenB(Choose(1e100,ChrW(55296),1/0))",
            "LenB(Switch(True,ChrW(55296),CVErr(1),1/0))",
            "Nz('ok',ChrW(65536))",
            "Choose(1,ChrW(55296),ChrB(65))",
            "LenB(Switch(True,ChrW(55296),'True',ChrB(65)))",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, success=False)
            controls += 1

    print(f"Passed {controls} independent raw String Nz/Switch/Choose controls; native Access unrun.")


if __name__ == "__main__":
    main()
