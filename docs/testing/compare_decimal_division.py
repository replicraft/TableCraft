"""Independent exact-ratio IEEE checks for explicit declared Decimal division.

This executes original SQL expressions, not imported database content. Microsoft
operator rules guide the engineering profile; Windows Access remains unqualified.
"""

import argparse
import csv
from fractions import Fraction
import hashlib
from io import StringIO
import json
from pathlib import Path
import struct
import subprocess
import sys


FRACTION_BITS = 52
EXPONENT_BITS = 11
EXPONENT_BIAS = (1 << (EXPONENT_BITS - 1)) - 1
MIN_NORMAL_EXPONENT = 1 - EXPONENT_BIAS
MIN_SUBNORMAL_EXPONENT = MIN_NORMAL_EXPONENT - FRACTION_BITS
SIGN_BIT = 1 << (FRACTION_BITS + EXPONENT_BITS)
HIDDEN_BIT = 1 << FRACTION_BITS
BASELINE_SHA256 = "820562569b83061a0129c9140ac95f4225849f404c831d4718202c0e40dd1b46"
REFERENCES = [
    "https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/"
    "ms-vbal/8628f21c-3f02-42b1-908c-201bd7ffe1b5",
    "https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/"
    "ms-vbal/56fb78c3-a0ea-4bf0-8ee4-eb5f89bda4f7",
]


def nearest_integer(numerator, denominator):
    quotient, remainder = divmod(numerator, denominator)
    twice = remainder * 2
    return quotient + (twice > denominator or (twice == denominator and quotient % 2 == 1))


def scaled_round(numerator, denominator, shift):
    if shift >= 0:
        numerator <<= shift
    else:
        denominator <<= -shift
    return nearest_integer(numerator, denominator)


def binary64(value, negative_zero=False):
    """Round an exact rational to IEEE binary64 using integer operations only."""
    sign = SIGN_BIT if value < 0 or (value == 0 and negative_zero) else 0
    numerator = abs(value.numerator)
    denominator = value.denominator
    if numerator == 0:
        return sign

    exponent = numerator.bit_length() - denominator.bit_length()
    if exponent >= 0:
        below = numerator < denominator << exponent
    else:
        below = numerator << -exponent < denominator
    exponent -= below

    if exponent < MIN_NORMAL_EXPONENT:
        significand = scaled_round(numerator, denominator, -MIN_SUBNORMAL_EXPONENT)
        return sign | significand

    significand = scaled_round(numerator, denominator, FRACTION_BITS - exponent)
    if significand == HIDDEN_BIT * 2:
        significand >>= 1
        exponent += 1
    if exponent > EXPONENT_BIAS:
        raise OverflowError("binary64 overflow")
    return sign | ((exponent + EXPONENT_BIAS) << FRACTION_BITS) | (significand - HIDDEN_BIT)


def exact_bits(bits):
    exponent = (bits >> FRACTION_BITS) & ((1 << EXPONENT_BITS) - 1)
    significand = bits & (HIDDEN_BIT - 1)
    power = MIN_SUBNORMAL_EXPONENT
    if exponent:
        significand += HIDDEN_BIT
        power = exponent - EXPONENT_BIAS - FRACTION_BITS
    result = Fraction(significand)
    result = result * (1 << power) if power >= 0 else result / (1 << -power)
    return -result if bits & SIGN_BIT else result


def division(left, right, left_negative_zero=False, right_negative_zero=False):
    left_bits = binary64(left, left_negative_zero)
    right_bits = binary64(right, right_negative_zero)
    numerator = exact_bits(left_bits)
    denominator = exact_bits(right_bits)
    if denominator == 0:
        return "overflow" if numerator == 0 else "division by zero"
    try:
        negative = bool((left_bits ^ right_bits) & SIGN_BIT)
        return binary64(numerator / denominator, negative)
    except OverflowError:
        return "overflow"


def model_controls():
    goldens = [
        (Fraction(1), False, 0x3FF0000000000000),
        (Fraction(1, 10), False, 0x3FB999999999999A),
        (Fraction(1, 3), False, 0x3FD5555555555555),
        (Fraction(1, 1 << 1074), False, 1),
        (Fraction(1, 1 << 1075), False, 0),
        (Fraction(3, 1 << 1075), False, 2),
        (-Fraction(1, 1 << 1075), False, SIGN_BIT),
        (Fraction(0), True, SIGN_BIT),
        (Fraction(((1 << 53) - 1) << 971), False, 0x7FEFFFFFFFFFFFFF),
    ]
    for value, negative_zero, expected in goldens:
        assert binary64(value, negative_zero) == expected
    try:
        binary64(Fraction((((1 << 53) - 1) << 971) + (1 << 970)))
    except OverflowError:
        pass
    else:
        raise AssertionError("overflow midpoint must round outside finite binary64")
    assert division(Fraction(3, 10), Fraction(1, 10)) == 0x4007FFFFFFFFFFFF
    return len(goldens) + 2


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--source", type=Path, action="append", default=[])
    parser.add_argument("--expect-sha256")
    parser.add_argument("--baseline-cli", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    model_count = model_controls()
    if args.self_test:
        print(f"Passed {model_count} independent IEEE model controls")
        return

    script = Path(__file__).resolve().relative_to(Path.cwd())
    paths = [args.cli, script, *args.source]
    if args.baseline_cli is not None:
        assert digest(args.baseline_cli) == BASELINE_SHA256, "unexpected baseline executable"
        paths.append(args.baseline_cli)
    pinned = {str(path): digest(path) for path in paths}
    if args.expect_sha256 is not None:
        assert pinned[str(args.cli)] == args.expect_sha256, "unexpected executable hash"
    observations = []

    def execute(sql, declared=True, options=("--number-format", "invariant"), mode="run", binary=None):
        command = [str(binary or args.cli), "--format", "tsv", "query", mode, sql, *options]
        if declared:
            command.append("--declared-arithmetic")
        result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", check=False)
        observations.append({
            "command": command,
            "returncode": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
        })
        return result

    def rows(sql, expected, declared=True, options=("--number-format", "invariant"), mode="run"):
        result = execute(sql, declared, options, mode)
        assert result.returncode == 0 and not result.stderr, (sql, result)
        actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        assert actual == expected, (sql, actual, expected)

    def failure(expression, message, options=("--number-format", "invariant")):
        result = execute(f"SELECT {expression} AS V", options=options)
        assert result.returncode == 2 and not result.stdout, (expression, result)
        assert message in result.stderr.lower(), (expression, result.stderr, message)

    def floating(expression, expected, options=("--number-format", "invariant")):
        if isinstance(expected, str):
            failure(expression, expected, options)
            return
        sql = f"SELECT VarType({expression}) AS K,{expression} AS V"
        result = execute(sql, options=options)
        assert result.returncode == 0 and not result.stderr, (sql, result)
        actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        assert len(actual) == 2 and actual[0] == ["K", "V"] and actual[1][0] == "5", (sql, actual)
        bits = int.from_bytes(struct.pack(">d", float(actual[1][1])), "big")
        assert bits == expected, (sql, actual, hex(bits), hex(expected))

    decimals = [
        ("CDec(1)", Fraction(1)),
        ("CDec(3)", Fraction(3)),
        ("CDec('0.3')", Fraction(3, 10)),
        ("CDec('0.0000000000000000000000000001')", Fraction(1, 10**28)),
        ("CDec('79228162514264337593543950335')", Fraction((1 << 96) - 1)),
    ]
    peers = [
        ("CDbl(3)", Fraction(3)),
        ("CDbl(0.1)", Fraction(1, 10)),
        ("CDbl(1e-100)", Fraction(1, 10**100)),
        ("CDbl(1e100)", Fraction(10**100)),
        ("CCur(1.25)", Fraction(5, 4)),
        ("CCur(0.0001)", Fraction(1, 10000)),
        ("CDate(-1.25)", Fraction(-5, 4)),
        ("CDate(0.1)", Fraction(1, 10)),
        ("'3'", Fraction(3)),
        ("'0.1'", Fraction(1, 10)),
        ("'1e100'", Fraction(10**100)),
        ("'1e-100'", Fraction(1, 10**100)),
    ]
    mixed_expressions = []
    for decimal, a in decimals:
        for peer, b in peers:
            for reverse in [False, True]:
                left, right, x, y = (peer, decimal, b, a) if reverse else (decimal, peer, a, b)
                expected = division(x, y)
                expression = f"{left}/{right}"
                mixed_expressions.append(expression)
                floating(expression, expected)

    for peer, b in peers:
        for reverse in [False, True]:
            left, right, x, y = (peer, "CDec(3)", b, Fraction(3)) if reverse else (
                "CDec(3)", peer, Fraction(3), b
            )
            for left_variant, right_variant in [(True, False), (False, True), (True, True)]:
                a_sql = f"CVar({left})" if left_variant else left
                b_sql = f"CVar({right})" if right_variant else right
                floating(f"{a_sql}/{b_sql}", division(x, y))

    boundary_cases = [
        ("CDec(1)/CDbl(5e-324)", "overflow"),
        ("CDec(0)/CDbl(5e-324)", 0),
        ("CDec(0)/CDbl(-1)", SIGN_BIT),
        ("CDbl(-0.0)/CDec(3)", SIGN_BIT),
        ("CDbl(-5e-324)/CDec(10)", SIGN_BIT),
        ("CDec('0.0000000000000000000000000001')/CDbl(1.7976931348623157e308)", 0),
        ("CDec('79228162514264337593543950335')/CDbl(1e-300)", "overflow"),
    ]
    for expression, expected in boundary_cases:
        floating(expression, expected)

    for profile, text, value in [
        ("de-de", "1,25", Fraction(5, 4)),
        ("de-de", "1.234,5", Fraction(2469, 2)),
        ("en-us", "1,234.5", Fraction(2469, 2)),
    ]:
        for reverse in [False, True]:
            expression = f"'{text}'/CDec(3)" if reverse else f"CDec(3)/'{text}'"
            a, b = (value, Fraction(3)) if reverse else (Fraction(3), value)
            floating(expression, division(a, b), ("--number-format", profile))

    for peer in ["CSng(2)", "CByte(2)", "CInt(2)", "CLng(2)", "CLngLng(2)", "CDec(2)"]:
        for expression, expected in [(f"CDec(3)/{peer}", Fraction(3, 2)), (f"{peer}/CDec(2)", Fraction(1))]:
            for declared in [False, True]:
                result = execute(f"SELECT VarType({expression}) AS K,{expression} AS V", declared)
                actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                assert result.returncode == 0 and not result.stderr and actual[1][0] == "14", (expression, result)
                assert Fraction(actual[1][1]) == expected, (expression, actual)

    for peer in ["CDbl(0)", "CCur(0)", "CDate(0)", "'0'"]:
        failure(f"CDec(0)/{peer}", "overflow")
        failure(f"CDec(1)/{peer}", "division by zero")
        failure(f"{peer}/CDec(0)", "overflow")
    failure("CDec(0)/CSng(0)", "division by zero")
    failure("CDec(1)/'3'", "number-format", options=())
    failure("CDec(1)/'bad'", "type mismatch")
    failure("NULL/CVErr(7)", "type mismatch")
    failure("CVErr(7)/NULL", "type mismatch")
    failure("NULL/(CDec(0)/CDbl(0))", "overflow")
    failure("(CDec(1)/CDbl(0))/(CDec(0)/CDbl(0))", "division by zero")
    failure("(CDec(0)/CDbl(0))/(CDec(1)/CDbl(0))", "overflow")
    failure("'bad'/(CDec(0)/CDbl(0))", "overflow")
    for expression in ["NULL/'bad'", "'bad'/NULL"]:
        rows(f"SELECT VarType({expression}) AS K,{expression} AS V", [["K", "V"], ["1", "NULL"]], options=())

    # The materialized profile remains Decimal-first in both public commands.
    for mode in ["run", "eval"]:
        for expression in ["CDec(3)/CDbl(2)", "CDec(3)/CCur(2)", "CDec(3)/CDate(2)", "CDec(3)/'2'"]:
            result = execute(f"SELECT VarType({expression}) AS K,{expression} AS V", False, mode=mode)
            actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert result.returncode == 0 and not result.stderr and actual[1][0] == "14", (expression, result)
            assert Fraction(actual[1][1]) == Fraction(3, 2), actual

    union = "(SELECT CDec(3)/CDbl(2) AS V UNION ALL SELECT CLng(2)) AS Q"
    for declared, tag in [(False, "14"), (True, "5")]:
        result = execute(f"SELECT VarType(V) AS K,V FROM {union}", declared)
        actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        assert result.returncode == 0 and not result.stderr and actual[0] == ["K", "V"], result
        assert [row[0] for row in actual[1:]] == [tag, tag], actual
        assert [Fraction(row[1]) for row in actual[1:]] == [Fraction(3, 2), Fraction(2)], actual
        for expression in ["Sum(V)", "IIf(True,V,CDec(0))"]:
            result = execute(f"SELECT VarType({expression}) AS K,{expression} AS V FROM {union}", declared)
            actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert result.returncode == 0 and not result.stderr, result
            assert all(row[0] == tag for row in actual[1:]), actual
            expected = [Fraction(7, 2)] if expression == "Sum(V)" else [Fraction(3, 2), Fraction(2)]
            assert [Fraction(row[1]) for row in actual[1:]] == expected, actual

    # Positive argument values exercise runtime refusal and buffered publication.
    for option in ["--execution-rows", "--execution-steps", "--output-bytes"]:
        result = execute(f"SELECT V FROM {union}", options=("--number-format", "invariant", option, "1"))
        assert result.returncode == 2 and not result.stdout, (option, result)

    independent_controls = len(observations)
    baseline_controls = 0
    if args.baseline_cli is not None:
        for expression in mixed_expressions:
            sql = f"SELECT VarType({expression}) AS K,{expression} AS V"
            old = execute(sql, declared=False, binary=args.baseline_cli)
            new = execute(sql, declared=False)
            assert (old.returncode, old.stdout, old.stderr) == (new.returncode, new.stdout, new.stderr), (
                expression, old, new
            )
            baseline_controls += 1

    for path, expected in pinned.items():
        assert digest(Path(path)) == expected, path
    report = {
        "scope": "Independent exact-ratio IEEE model; explicit declared profile, native Access unqualified",
        "references": REFERENCES,
        "runner_arguments": sys.argv[1:],
        "model_controls": model_count,
        "cli_invocations": len(observations),
        "independent_cli_controls": independent_controls,
        "observational_default_pairs": baseline_controls,
        "unchanged_hashes": pinned,
        "observations": observations,
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(
        f"Passed {independent_controls} independent CLI controls, {baseline_controls} default baseline pairs "
        f"and {model_count} IEEE model controls"
    )


if __name__ == "__main__":
    main()
