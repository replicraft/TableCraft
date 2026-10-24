"""Independent checks of the explicit declared-arithmetic decimal literal policy.

Python integers and IEEE binary64 conversions supply the expected values. This is
engineering evidence, not native Microsoft Access execution qualification.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import struct
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    args = parser.parse_args()
    binary = args.cli.resolve()
    before = hashlib.sha256(binary.read_bytes()).hexdigest()
    controls = 0
    refusals = 0

    def check(expression, expected, tag, *, declared=True):
        nonlocal controls
        sql = f"SELECT {expression} AS N,VarType({expression}) AS T"
        command = [str(binary), "query", "run", sql, "--format", "tsv"]
        if declared:
            command.append("--declared-arithmetic")
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        assert result.returncode == 0 and not result.stderr, (sql, result)
        rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        assert len(rows) == 2 and rows[0] == ["N", "T"], (sql, rows)
        assert rows[1][1] == str(tag), (sql, rows, tag)
        if tag == 5:
            assert struct.pack(">d", float(rows[1][0])) == struct.pack(">d", expected), (sql, rows, expected)
        else:
            assert rows[1][0] == str(expected), (sql, rows, expected)
        controls += 1

    def refusal(expression, error):
        nonlocal refusals
        result = subprocess.run(
            [str(binary), "query", "run", f"SELECT {expression}",
             "--declared-arithmetic", "--format", "tsv"],
            capture_output=True, text=True, check=False,
        )
        assert result.returncode == 2 and not result.stdout, (expression, result)
        assert error in result.stderr, (expression, result.stderr, error)
        refusals += 1

    magnitudes = [
        0, 1, 32766, 32767, 32768, 32769,
        (1 << 31) - 1, 1 << 31, (1 << 31) + 1,
        (1 << 53) - 1, 1 << 53, (1 << 53) + 1,
        (1 << 63) - 1, 1 << 63, (1 << 63) + 1,
        79228162514264337593543950335,
    ]
    for magnitude in magnitudes:
        tag = 2 if magnitude <= 32767 else 3 if magnitude <= 2147483647 else 5
        for sign in [1, -1]:
            value = sign * magnitude
            expected = float(value) if tag == 5 else value
            spelling = str(value)
            for expression in [spelling, f"({spelling})", f"CVar({spelling})"]:
                check(expression, expected, tag)
            check(f"{spelling}.0", float(value), 5)
            check(f"{spelling}e0", float(value), 5)
            if -(1 << 63) <= value < (1 << 63):
                check(spelling, value, 20, declared=False)

    for maximum, next_tag in [(32767, 3), (2147483647, 5)]:
        refusal(f"{maximum}+1", "numeric overflow")
        value = maximum + 1
        check(f"CVar({maximum})+1", float(value) if next_tag == 5 else value, next_tag)
        check(f"IIf(True,{maximum},0)+1", float(value) if next_tag == 5 else value, next_tag)
    for expression in ["IIf(True,1,1/0)", "Null+(1/0)"]:
        refusal(expression, "division by zero")
    refusal("CLngLng(9223372036854775807)", "numeric overflow")
    refusal("9" * 129, "literal")

    after = hashlib.sha256(binary.read_bytes()).hexdigest()
    assert before == after, (before, after)
    print(f"PASS: {controls} value/subtype controls; {refusals} refusals; binary SHA256 {before}")


if __name__ == "__main__":
    main()
