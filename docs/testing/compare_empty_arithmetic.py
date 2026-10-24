"""Independent integer/Empty models for the explicit host API probe.

This checks caller-issued declaration categories, not SQL Empty construction,
native Access execution or the validity of a native static Empty declaration.
"""
import argparse
import itertools
from pathlib import Path
import random
import subprocess

from compare_declared_integral import BOUNDS, model


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", type=Path, required=True)
    args = parser.parse_args()
    rng = random.Random(20261024)
    controls = 0

    def check(operator, left, right, width, a, b):
        nonlocal controls
        for left_declaration, right_declaration in itertools.product(["scalar", "variant"], repeat=2):
            expected = model(operator, a, b, width, "variant" in [left_declaration, right_declaration])
            result = subprocess.run([
                str(args.probe), "--operator", operator, "--left", left, "--right", right,
                "--left-declaration", left_declaration, "--right-declaration", right_declaration,
            ], capture_output=True, text=True, check=False)
            if expected is None:
                assert result.returncode == 2 and not result.stdout, (operator, left, right, result)
            else:
                assert result.returncode == 0 and result.stdout == expected and not result.stderr, (
                    operator, left, right, result, expected,
                )
            controls += 1

    for kind, (low, high) in BOUNDS.items():
        values = [low, high, 0, 1]
        values += [rng.randint(low, high) for _ in range(16)]
        for value in values:
            for operator in ["add", "subtract", "multiply", "integer-divide", "modulo"]:
                # Empty/Byte arithmetic keeps Byte, except rounded quotient and
                # remainder which explicitly select Integer. Other widths retain
                # the integral counterpart's type.
                width = "i16" if kind == "byte" and operator in ["integer-divide", "modulo"] else kind
                typed = f"{kind}:{value}"
                check(operator, "empty", typed, width, 0, value)
                check(operator, typed, "empty", width, value, 0)

    for operator in ["add", "subtract", "multiply", "integer-divide", "modulo"]:
        check(operator, "empty", "empty", "i16", 0, 0)
        for left, right in [("empty", "null"), ("null", "empty")]:
            result = subprocess.run([
                str(args.probe), "--operator", operator, "--left", left, "--right", right,
                "--left-declaration", "variant", "--right-declaration", "variant",
            ], capture_output=True, text=True, check=False)
            assert result.returncode == 0 and result.stdout == "NULL\n" and not result.stderr, result
            controls += 1

    print(f"Passed {controls} independent Empty-integral API controls; SQL/native qualification separate.")


if __name__ == "__main__":
    main()
