"""Independent integer/declaration arithmetic models for the explicit API probe.

This exercises the shared kernel, not SQL declaration inference or native Access.
"""
import argparse
import itertools
from pathlib import Path
import random
import struct
import subprocess

BOUNDS = {"byte": (0, 255), "i16": (-32768, 32767), "i32": (-2147483648, 2147483647), "i64": (-9223372036854775808, 9223372036854775807)}
TAGS = {"byte": "Byte", "i16": "Integer", "i32": "Long", "i64": "LongLong"}


def target(a, b):
    if "i64" in [a, b]: return "i64"
    if "i32" in [a, b]: return "i32"
    return "byte" if a == b == "byte" else "i16"


def model(operation, a, b, width, variant):
    if operation in ["integer-divide", "modulo"]:
        if b == 0: return None
        quotient = (abs(a) // abs(b)) * (-1 if (a < 0) != (b < 0) else 1)
        result = quotient if operation == "integer-divide" else a - quotient * b
    elif operation == "add": result = a + b
    elif operation == "subtract": result = a - b
    else: result = a * b
    start = ["byte", "i16", "i32", "i64"].index(width)
    for candidate in ["byte", "i16", "i32", "i64"][start:]:
        if candidate == "i64" and width != "i64": break
        low, high = BOUNDS[candidate]
        if low <= result <= high: return f"{TAGS[candidate]}:{result}\n"
        if not variant: return None
    if not variant or width == "i64": return None
    return "Double:" + struct.pack("!d", float(result)).hex() + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", type=Path, required=True)
    args = parser.parse_args()
    rng = random.Random(20261022)
    boundaries = [(kind, value) for kind, (low, high) in BOUNDS.items() for value in [low, high, 0, 1]]
    pairs = list(itertools.product(boundaries, repeat=2))
    pairs += [(('bool', -1), ('byte', 255)), (('bool', 0), ('i16', -32768))]
    for _ in range(128):
        a, b = rng.choices(list(BOUNDS), k=2)
        pairs.append(((a, rng.randint(*BOUNDS[a])), (b, rng.randint(*BOUNDS[b]))))
    controls = 0
    for (a_kind, a), (b_kind, b) in pairs:
        left = f"{a_kind}:{str(bool(a)).lower() if a_kind == 'bool' else a}"
        right = f"{b_kind}:{str(bool(b)).lower() if b_kind == 'bool' else b}"
        for operation in ["add", "subtract", "multiply", "integer-divide", "modulo"]:
            for left_declaration, right_declaration in itertools.product(["scalar", "variant"], repeat=2):
                expected = model(operation, a, b, target(a_kind, b_kind), "variant" in [left_declaration, right_declaration])
                process = subprocess.run([str(args.probe), "--operator", operation, "--left", left,
                    "--right", right, "--left-declaration", left_declaration, "--right-declaration", right_declaration], capture_output=True, text=True, check=False)
                if expected is None:
                    assert process.returncode == 2 and not process.stdout, (left, right, operation, process)
                else:
                    assert process.returncode == 0 and process.stdout == expected and not process.stderr, (left, right, operation, process, expected)
                controls += 1
    for operation, a, b, result in [("divide",3,2,1.5), ("power",2,3,8.0)]:
        for left_declaration, right_declaration in itertools.product(["scalar", "variant"], repeat=2):
            process = subprocess.run([str(args.probe), "--operator", operation, "--left", f"i16:{a}", "--right", f"i16:{b}", "--left-declaration", left_declaration, "--right-declaration", right_declaration], capture_output=True, text=True, check=False)
            assert process.returncode == 0 and process.stdout == "Double:" + struct.pack("!d",result).hex() + "\n"
            controls += 1
    print(f"Passed {controls} independent declared-integral API controls; SQL/native Access qualification remains separate")


if __name__ == "__main__":
    main()
