"""Exact-rational 96-bit Decimal arithmetic controls; native Access is unrun.

Python Fraction supplies exact answers and ties-to-even quantization. No Decimal
library, reference SQL engine, database fixture or binary float supplies answers.
"""

import argparse
import csv
from fractions import Fraction
import hashlib
import io
import json
from pathlib import Path
import random
import subprocess


MAXIMUM = (1 << 96) - 1
MAX_SCALE = 28
BATCH_SIZE = 4
SEED = 20261024


def literal(coefficient, scale):
    digits = str(abs(coefficient)).rjust(scale + 1, "0")
    if scale:
        digits = digits[:-scale] + "." + digits[-scale:]
    return ("-" if coefficient < 0 else "") + digits


def quantize(value):
    # Derive each candidate directly from the exact rational result. Repeated
    # rounding from a previously shortened coefficient can introduce a second
    # rounding error. Representational scale is deliberately not prescribed.
    for scale in range(MAX_SCALE, -1, -1):
        coefficient = round(value * 10 ** scale)
        if abs(coefficient) <= MAXIMUM:
            return Fraction(coefficient, 10 ** scale)
    return None


def operands():
    pairs = []
    for scale in range(MAX_SCALE + 1):
        for sign in [-1, 1]:
            for offset in [0, 1, 5]:
                left = (sign * (MAXIMUM - offset), scale)
                for right in [(5, 1), (-5, 1), (15, 1), (1, 28)]:
                    pairs.append((left, right))
        for other_scale in range(MAX_SCALE + 1):
            pairs.append(((MAXIMUM, scale), (MAXIMUM - 1, other_scale)))
            pairs.append(((1, scale), (3, other_scale)))

    generator = random.Random(SEED)
    for _ in range(512):
        pair = []
        for _ in range(2):
            coefficient = generator.choice([-1, 1]) * generator.randrange(MAXIMUM + 1)
            pair.append((coefficient, generator.randrange(MAX_SCALE + 1)))
        pairs.append(tuple(pair))
    return pairs


def cases():
    result = []
    for left, right in operands():
        a = Fraction(left[0], 10 ** left[1])
        b = Fraction(right[0], 10 ** right[1])
        operations = [("+", a + b), ("-", a - b), ("*", a * b)]
        if b:
            operations.append(("/", a / b))
        for operator, exact in operations:
            sql = f"CDec('{literal(*left)}'){operator}CDec('{literal(*right)}')"
            result.append((sql, quantize(exact)))
    return result


def execute(cli, mode, expressions, diagnostic=None):
    sql = "SELECT " + ",".join(expressions)
    result = subprocess.run(
        [str(cli), "query", mode, sql, "--number-format", "invariant", "--format", "tsv"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=False,
    )
    if diagnostic is not None:
        assert result.returncode == 2 and not result.stdout, (mode, sql, result)
        assert diagnostic in result.stderr, (mode, sql, result.stderr)
        return None

    assert result.returncode == 0 and not result.stderr, (mode, sql, result.stderr)
    rows = list(csv.reader(io.StringIO(result.stdout), delimiter="\t", strict=True))
    assert len(rows) == 2 and len(rows[1]) == len(expressions), (sql, rows)
    return rows[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--report", type=Path, help="Fresh JSON evidence file; existing files are refused")
    args = parser.parse_args()
    if args.report is not None and args.report.exists():
        parser.error("--report must not exist")

    binary_hash = hashlib.sha256(args.cli.read_bytes()).hexdigest()
    controls = {"successful_results": 0, "overflow_refusals": 0, "result_subtypes": 0,
                "zero_divisors": 0, "null_results": 0, "eager_failures": 0}
    examples = cases()
    successes = [(sql, value) for sql, value in examples if value is not None]
    overflows = [sql for sql, value in examples if value is None]

    for mode in ["eval", "run"]:
        for start in range(0, len(successes), BATCH_SIZE):
            batch = successes[start:start + BATCH_SIZE]
            actual = execute(args.cli, mode, [sql for sql, _ in batch])
            for (sql, wanted), value in zip(batch, actual, strict=True):
                assert Fraction(value) == wanted, (mode, sql, value, wanted)
                controls["successful_results"] += 1

        for sql in overflows:
            execute(args.cli, mode, ["1", sql], diagnostic="overflow")
            controls["overflow_refusals"] += 1

        for position in range(0, len(successes), 97):
            sql, _ = successes[position]
            assert execute(args.cli, mode, [f"VarType({sql})"]) == ["14"]
            controls["result_subtypes"] += 1

        for scale in range(MAX_SCALE + 1):
            zero = f"CDec('{literal(0, scale)}')"
            execute(args.cli, mode, ["1", f"CDec('1')/{zero}"], diagnostic="division by zero")
            controls["zero_divisors"] += 1
            assert execute(args.cli, mode, [f"NULL/{zero}"]) == ["NULL"]
            controls["null_results"] += 1

        for sql in ["CDec('1')/(CDec('0')+1/0)", "NULL*(CDec('1')/CDec('0'))"]:
            execute(args.cli, mode, ["1", sql], diagnostic="division by zero")
            controls["eager_failures"] += 1

    assert hashlib.sha256(args.cli.read_bytes()).hexdigest() == binary_hash
    report = {
        "scope": "96-bit fixed Decimal arithmetic; exact rational numeric answers; no scale identity claim",
        "seed": SEED,
        "operand_pairs": len(operands()),
        "binary_operations": len(examples),
        "modes": ["eval", "run"],
        "controls": controls,
        "total_controls": sum(controls.values()),
        "binary_sha256": binary_hash,
        "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "native_access": "unrun",
    }
    if args.report is not None:
        with args.report.open("x", encoding="utf-8") as output:
            json.dump(report, output, indent=2)
            output.write("\n")
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
