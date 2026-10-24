"""Independent exact Fraction/half-even Decimal SUM controls; native Access unrun."""

import argparse
import csv
from fractions import Fraction
import hashlib
from io import StringIO
import itertools
import json
from pathlib import Path
import random
import subprocess

MAXIMUM = (1 << 96) - 1


def literal(coefficient, scale):
    digits = str(abs(coefficient)).rjust(scale + 1, "0")
    if scale:
        digits = digits[:-scale] + "." + digits[-scale:]
    return ("-" if coefficient < 0 else "") + digits


def quantize(value):
    for scale in range(28, -1, -1):
        # Derive each candidate afresh from the exact rational total; rounding a
        # previously rounded candidate can cross a later halfway boundary.
        coefficient = round(value * 10 ** scale)
        if abs(coefficient) <= MAXIMUM:
            return Fraction(coefficient, 10 ** scale)
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    binary = args.cli.resolve()
    before = hashlib.sha256(binary.read_bytes()).hexdigest()
    controls = 0
    refusals = 0
    query_digest = hashlib.sha256()

    def execute(values, expression="Sum(Amount)", error=None):
        nonlocal controls, refusals
        inputs = " UNION ALL ".join(
            ("SELECT NULL AS Amount" if value is None else
             f"SELECT CDec('{literal(*value)}') AS Amount") + f",{index} AS ID"
            for index, value in enumerate(values)
        )
        sql = f"SELECT {expression} AS S,VarType({expression}) AS T FROM ({inputs}) AS Inputs"
        query_digest.update(sql.encode())
        query_digest.update(b"\n")
        result = subprocess.run(
            [str(binary), "query", "run", sql, "--number-format", "invariant", "--format", "tsv"],
            capture_output=True, text=True, check=False,
        )
        present = [Fraction(value, 10 ** scale) for item in values if item is not None
                   for value, scale in [item]]
        expected = quantize(sum(present, Fraction())) if present else None
        overflow = bool(present) and expected is None
        if error is not None or overflow:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert (error or "numeric overflow") in result.stderr, (sql, result.stderr)
            refusals += 1
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert len(rows) == 2 and rows[0] == ["S", "T"], (sql, rows)
            if not present:
                assert rows[1] == ["NULL", "1"], (sql, rows)
            else:
                assert Fraction(rows[1][0]) == expected and rows[1][1] == "14", (
                    sql, rows, expected,
                )
            controls += 1

    cases = [[None], [(125, 2), (250, 2)]]
    for residual in [1, -1]:
        cases.extend(itertools.permutations([(MAXIMUM, 0), (residual, 28), (-MAXIMUM, 0)]))
    cases.extend(itertools.permutations([(MAXIMUM, 0), (MAXIMUM, 0), (-MAXIMUM, 0)]))
    for scale in [0, 1, 14, 28]:
        for sign in [-1, 1]:
            cases.append([(sign * MAXIMUM, scale)] * 9 + [None])
    for sign in [-1, 1]:
        for base in [MAXIMUM - 1, MAXIMUM - 2]:
            cases.append([(sign * base, 1), (sign * 5, 2)])
    generator = random.Random(20261024)
    for _ in range(96):
        cases.append([
            (generator.choice([-1, 1]) * generator.randrange(MAXIMUM // 2, MAXIMUM),
             generator.randrange(29))
            for _ in range(generator.randrange(2, 10))
        ])
    for values in cases:
        execute(values)
    execute([(MAXIMUM, 0), (MAXIMUM, 0), (1, 0)],
            "Sum(IIf(ID=2,1/(2-ID),Amount))", "division by zero")
    execute([(MAXIMUM, 0), (MAXIMUM, 0), (1, 0)],
            "Sum(IIf(ID=2,CVErr(7),Amount))", "type mismatch")

    after = hashlib.sha256(binary.read_bytes()).hexdigest()
    assert before == after, (before, after)
    summary = f"Passed {controls} Fraction/half-even Decimal sum results and {refusals} refusals; binary SHA256 {before}"
    if args.report:
        args.report.write_text(json.dumps({
            "date": "2026-10-24",
            "scope": "Source-free wider Decimal SUM final rounding, cancellation residuals and eager late failures",
            "native_access_qualification": "unrun",
            "binary_sha256_before": before,
            "binary_sha256_after": after,
            "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "generated_sql_sha256": query_digest.hexdigest(),
            "result_controls": controls,
            "refusal_controls": refusals,
            "reference": "Python Fraction exact totals and independent per-scale half-even round; no production imports",
            "stdout": summary + "\n",
        }, indent=2) + "\n")
    print(summary)


if __name__ == "__main__":
    main()
