"""Independent SQL integral bounds and declaration-propagation controls.

This checks the explicit engineering policy, not Microsoft Access equivalence.
"""

import argparse
import itertools
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    controls = 0

    def check(expression, expected, tag=None, declared=True):
        nonlocal controls
        sql = f"SELECT {expression} AS N"
        if tag is not None:
            sql += f",VarType({expression}) AS T"
        command = [str(args.cli.resolve()), "query", "run", sql, "--format", "tsv"]
        if declared:
            command.append("--declared-integral")
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        if expected is None:
            assert result.returncode == 2 and not result.stdout, (sql, result)
        else:
            output = f"N\n{expected}\n" if tag is None else f"N\tT\n{expected}\t{tag}\n"
            assert result.returncode == 0 and result.stdout == output and not result.stderr, (
                sql, result, output
            )
        controls += 1

    widths = [("CByte", 0, 255, 17), ("CInt", -32768, 32767, 2),
              ("CLng", -2147483648, 2147483647, 3)]
    for cast, low, high, tag in widths:
        pairs = [(high, 1), (high, 2), (low, 1), (low, -1 if low < 0 else 2),
                 (0, 0), (7, 2)]
        for (left, right), operator, variant in itertools.product(
                pairs, ["+", "-", "*", "\\", "Mod"], [False, True]):
            a, b = f"{cast}({left})", f"{cast}({right})"
            if variant:
                a = f"CVar({a})"
            expression = f"{a} {operator} {b}"
            if operator in ["\\", "Mod"]:
                if right == 0:
                    check(expression, None)
                    continue
                quotient = abs(left) // abs(right)
                if (left < 0) != (right < 0):
                    quotient = -quotient
                value = quotient if operator == "\\" else left - quotient * right
            elif operator == "+":
                value = left + right
            elif operator == "-":
                value = left - right
            else:
                value = left * right
            if low <= value <= high:
                check(expression, value, tag)
            elif not variant:
                check(expression, None)
            elif -32768 <= value <= 32767:
                check(expression, value, 2)
            elif -2147483648 <= value <= 2147483647:
                check(expression, value, 3)
            else:
                check(expression, value, 5)

    for wrapper in ["CVar({})", "IIf(True,{},CInt(0))", "Nz({},CInt(0))",
                    "Choose(1,{})", "Switch(True,{})"]:
        value = wrapper.format("CInt(32767)")
        check(f"{value}+CInt(1)", 32768, 3)
        check(f"(SELECT Q.N+CInt(1) FROM (SELECT {value} AS N) AS Q)", 32768, 3)
    check("CInt(32767)+CInt(1)", 32768, 3, declared=False)
    check("-CInt(-32768)", None)
    check("-CVar(CInt(-32768))", 32768, 3)
    check("First(CInt(1))+CInt(1)", None)
    check("CVar(NULL)+(1/0)", None)
    print(f"Passed {controls} independent SQL declaration controls; native Access remains unqualified")


if __name__ == "__main__":
    main()
