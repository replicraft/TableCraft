"""Original Power boundary replay; no production imports or arbitrary libm oracle.

Bounded integral exponents use exact Fraction arithmetic on independently coerced
binary64 operands. Only exact dyadic powers and finite integer goldens are used;
this is not a correctly-rounded arbitrary transcendental pow implementation.
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


ROOT = Path(__file__).resolve().parents[2]
REFERENCE = (
    "https://learn.microsoft.com/en-us/openspecs/"
    "microsoft_general_purpose_programming_languages/ms-vbal/"
    "c3f526af-9d58-47f9-a6be-22ab81e8747a"
)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def bits(value):
    return int.from_bytes(struct.pack(">d", value), "big")


def integer_power(base, exponent):
    """Round input to Double before exact rational exponentiation."""
    converted = float(base)
    if converted == 0 and exponent < 0:
        return "domain"
    result = Fraction.from_float(converted) ** exponent
    try:
        rounded = float(result)
    except OverflowError:
        return "overflow"
    if result == 0 and converted < 0 and exponent % 2:
        rounded = -0.0
    if rounded == 0 and result < 0:
        rounded = -0.0
    return bits(rounded)


def self_test():
    controls = [
        (Fraction(0), -1, "domain"),
        (Fraction(0), 0, 0x3FF0000000000000),
        (Fraction(-2), -3, 0xBFC0000000000000),
        (Fraction(1, 2), 1075, 0),
        (Fraction(-1, 2), 1075, 0x8000000000000000),
        (Fraction(2), 1024, "overflow"),
        (Fraction(3, 2), 3, 0x400B000000000000),
    ]
    for base, exponent, expected in controls:
        assert integer_power(base, exponent) == expected
    return len(controls)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--source", type=Path, action="append", default=[])
    parser.add_argument("--expect-sha256")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    controls = self_test()
    if args.self_test:
        print(f"Passed {controls} independent Fraction model controls")
        return
    if args.cli is None or args.report is None:
        parser.error("--cli and --report are required for replay")

    def local_path(path):
        absolute = path.resolve() if path.is_absolute() else (ROOT / path).resolve()
        try:
            return absolute.relative_to(ROOT)
        except ValueError:
            parser.error("all replay paths must remain inside the repository")

    args.cli = local_path(args.cli)
    args.report = local_path(args.report)
    args.source = [local_path(path) for path in args.source]
    if (ROOT / args.report).exists():
        parser.error("report already exists; preserve the prior attempt and select a fresh path")

    paths = [args.cli, local_path(Path(__file__)), *args.source]
    before = {str(path): digest(ROOT / path) for path in paths}
    if args.expect_sha256:
        assert before[str(args.cli)] == args.expect_sha256
    observations = []

    def observe(expression, expected, declared, mode="run", options=("--number-format", "invariant")):
        sql = f"SELECT {expression} AS V"
        if isinstance(expected, int):
            sql = f"SELECT {expression} AS V,VarType({expression}) AS K"
        command = [str(args.cli), "--format", "tsv", "query", mode, sql, *options]
        if declared:
            command.append("--declared-arithmetic")
        result = subprocess.run(
            command,
            capture_output=True,
            text=True,
            check=False,
            timeout=30,
            cwd=ROOT,
        )
        item = {
            "command": command,
            "expected": expected,
            "returncode": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
        }
        observations.append(item)
        if isinstance(expected, str) and expected != "NULL":
            item["agrees"] = (
                result.returncode == 2 and not result.stdout and expected in result.stderr.lower()
            )
        elif result.returncode == 0 and not result.stderr:
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            if expected == "NULL":
                item["agrees"] = rows == [["V"], ["NULL"]]
            else:
                item["agrees"] = (
                    len(rows) == 2 and rows[0] == ["V", "K"] and rows[1][1] == "5"
                    and bits(float(rows[1][0])) == expected
                )
        else:
            item["agrees"] = False

    bases = [
        "0",
        "False",
        "CByte(0)",
        "CInt(0)",
        "CLng(0)",
        "CLngLng(0)",
        "CSng(0)",
        "CDbl(0)",
        "CCur(0)",
        "CDec(0)",
        "CDate(0)",
        "'0'",
        "LeftB('0',2)",
        "(-CDbl(0))",
        "(0.5^1075)",
    ]
    exponents = [
        "(-1)",
        "True",
        "CInt(-1)",
        "CLng(-1)",
        "CLngLng(-1)",
        "CSng(-1)",
        "CDbl(-1)",
        "CCur(-1)",
        "CDec(-1)",
        "CDate(-1)",
        "'-1'",
        "LeftB('-1',4)",
        "(-0.5)",
    ]
    for declared in [False, True]:
        for base in bases:
            for exponent in exponents:
                observe(f"({base})^({exponent})", "domain", declared)
        for mode in (["run"] if declared else ["run", "eval"]):
            for base in [
                Fraction(-4),
                Fraction(-2),
                Fraction(-1, 2),
                Fraction(0),
                Fraction(1, 2),
                Fraction(2),
                Fraction(4),
            ]:
                for exponent in [-8, -3, -1, 0, 1, 2, 3, 8]:
                    literal = str(float(base))
                    observe(f"({literal})^({exponent})", integer_power(base, exponent), declared, mode)
            for expression, expected in [
                ("(-0.5)^1075", 0x8000000000000000),
                ("0.5^1075", 0),
                ("(-CDbl(0))^3", 0x8000000000000000),
                ("(-1)^CLngLng('9007199254740993')", 0x3FF0000000000000),
                ("(-1)^CDec('9007199254740993')", 0x3FF0000000000000),
                ("(-1)^'9007199254740993'", 0x3FF0000000000000),
                ("4^0.5", 0x4000000000000000),
                ("(-2)^0.5", "domain"),
                ("2^1024", "overflow"),
                ("0^(1/0)", "division by zero"),
                ("NULL^(1/0)", "division by zero"),
                ("(0^(-1))^(1/0)", "domain"),
                ("(1/0)^(0^(-1))", "division by zero"),
                ("CVErr(1)^0", "type mismatch"),
                ("0^CVErr(1)", "type mismatch"),
                ("0^ChrB(65)", "utf-16"),
                ("'bad'^(-1)", "type mismatch"),
                ("0/0", "overflow"),
                ("CDec(0)/0", "division by zero"),
                ("NULL^(-1)", "NULL"),
                ("0^NULL", "NULL"),
                ("ChrB(65)^NULL", "NULL"),
            ]:
                observe(expression, expected, declared, mode)
            observe("'0'^(-1)", "requires an explicit number-format policy", declared, mode, ())

    after = {str(path): digest(ROOT / path) for path in paths}
    assert before == after, "binary, replay or selected source changed during run"
    report = {
        "scope": "TableCraft Power contract; no Windows Access qualification or arbitrary libm oracle",
        "reference": REFERENCE,
        "model_controls": controls,
        "sha256_before": before,
        "sha256_after": after,
        "commands": len(observations),
        "disagreements": sum(not item["agrees"] for item in observations),
        "observations": observations,
    }
    (ROOT / args.report).write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: value for key, value in report.items() if key != "observations"}, indent=2))
    raise SystemExit(bool(report["disagreements"]))


if __name__ == "__main__":
    main()
