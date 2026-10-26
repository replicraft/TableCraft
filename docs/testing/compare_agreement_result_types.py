"""Check unknown-category arithmetic/UNION composition over original stored fields.

Only TableCraft executes SQL. Public native getters supply inputs and Python
integer/Fraction operations supply expected values, not native ACE result types.
"""

import argparse
import csv
from dataclasses import dataclass
from fractions import Fraction
import hashlib
import io
import json
from pathlib import Path
import subprocess

import compare_fixed_binary_keys as mixed
from compare_declared_numeric_text import INPUTS, JAR_SHA256

ROOT = Path(__file__).resolve().parents[2]
MAX_COMMANDS = 400
CANDIDATE_SHA256 = "5a73870b888d58a22314096e761ec3ddec8cfc4ea6bbf7a7e5acda868b46d10c"
BASELINE_SHA256 = "f9e51ed3b8bfd0500ed8f650cec6e2bdbf27ee4d93fccfab2e176454f6277433"
SOURCE_FILES = [
    "crates/query/tablecraft-query/src/binding/result_types.rs",
    "crates/query/tablecraft-query/src/binding/result_types/arithmetic.rs",
    "crates/query/tablecraft-query/src/binding/result_types/unary.rs",
    "crates/query/tablecraft-query/src/output_types/union.rs",
    "crates/query/tablecraft-query/src/execution/declarations.rs",
]


@dataclass(frozen=True)
class Case:
    name: str
    expression: str
    row: int
    tag: int
    value: Fraction | None
    derived: bool = False


def cases(rows):
    small = rows[1]["Small"].key
    positive = rows[2]["Small"].key
    single = rows[2]["Real"].key
    byte = rows[1]["Octet"].key
    assert (small, positive, single, byte) == (-32768, 32767, Fraction(3, 2), 255)
    assert rows[3]["Small"] is None
    return [
        Case("byte-add", "First([Octet])+CByte(0)", 1, 17, byte),
        Case("integer-add", "First([Small])+CInt(1)", 1, 2, small + 1),
        Case("integer-negate", "-First([Small])", 2, 2, -positive),
        Case("integer-divide", "First([Small])\\CInt(2)", 1, 2, small / 2),
        Case("single-multiply", "First([Real])*CSng(2)", 2, 4, single * 2),
        Case("single-divide", "First([Real])/CSng(2)", 2, 4, single / 2),
        Case("null-integer", "First([Small])+CInt(1)", 3, 1, None),
        Case("empty-integer", "First([Small])+CInt(1)", 99, 1, None),
        Case("nested-union-integer", "Q.N+CInt(1)", 1, 2, small + 1, True),
    ]


def sql(case, reverse):
    if case.derived:
        relation = f"SELECT [Small] AS N FROM ScalarSamples WHERE ID={case.row}"
        source = f"FROM ({relation} UNION ALL {relation} AND False) Q"
    else:
        source = f"FROM ScalarSamples WHERE ID={case.row}"
    selected = f"SELECT {case.expression} AS V {source}"
    other = "SELECT CCur(4) AS V"
    arms = f"{other} UNION ALL {selected}" if reverse else f"{selected} UNION ALL {other}"
    return f"SELECT V,VarType(V) AS T FROM ({arms}) W"


def expected(case, declared, baseline, reverse):
    tag = case.tag
    if declared and not baseline and case.value is not None:
        tag = 5 if tag == 4 else 6
    other_tag = 5 if declared and not baseline and case.tag == 4 else 6
    result = [(case.value, tag), (Fraction(4), other_tag)]
    return list(reversed(result)) if reverse else result


def check_table(stdout, wanted):
    table = list(csv.reader(io.StringIO(stdout), delimiter="\t", strict=True))
    if table[0] != ["V", "T"] or len(table) != len(wanted) + 1:
        raise AssertionError("Unexpected result shape")
    for row, (value, tag) in zip(table[1:], wanted, strict=True):
        if len(row) != 2 or row[1] != str(tag):
            raise AssertionError((row, value, tag))
        if value is None:
            if row[0] != "NULL":
                raise AssertionError("Expected NULL")
        elif Fraction(row[0]) != value:
            # Every binary value selected in this workload is an exact dyadic.
            raise AssertionError((row, value))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "baseline", "jar", "directory", "report"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    if args.report.exists():
        raise ValueError("Report must be fresh")
    if (mixed.digest(args.cli), mixed.digest(args.baseline), mixed.digest(args.jar)) != (
        CANDIDATE_SHA256, BASELINE_SHA256, JAR_SHA256
    ):
        raise ValueError("Executable/reference fingerprint differs")
    for name, fingerprint in INPUTS.items():
        if mixed.digest(args.directory / name) != fingerprint:
            raise ValueError("Original native fingerprint differs")
    revision = subprocess.run(
        ["git", "rev-parse", "--verify", f"{args.revision}^{{commit}}"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
    sources = {}
    for name in SOURCE_FILES:
        content = subprocess.run(
            ["git", "show", f"{revision}:{name}"], check=True, capture_output=True,
        ).stdout
        sources[name] = hashlib.sha256(content).hexdigest()
        if mixed.digest(ROOT / name) != sources[name]:
            raise ValueError("Selected source differs from revision")
    artifacts = [args.cli, args.baseline, args.jar, Path(__file__),
                 ROOT / "docs/testing/compare_fixed_binary_keys.py",
                 ROOT / "docs/testing/compare_declared_numeric_text.py",
                 ROOT / "docs/testing/compare_numeric_keys.py",
                 ROOT / "docs/testing/TableDataOracle.java",
                 *(args.directory / name for name in INPUTS)]
    pins = {str(path.resolve().relative_to(ROOT)): mixed.digest(path) for path in artifacts}
    report = {
        "date": "2026-10-25", "revision": revision, "scope": __doc__,
        "immutable_before": pins, "selected_source_sha256": sources,
        "getters": {}, "observations": [], "complete": False,
    }

    def run(database, label, text, declared, baseline, wanted=None):
        if len(report["observations"]) >= MAX_COMMANDS:
            raise ValueError("Command ceiling exceeded")
        binary = args.baseline if baseline else args.cli
        command = [str(binary), "--format", "tsv", "query", "run", text,
                   "--database", str(database), "--binding-depth", "32"]
        if declared:
            command.append("--declared-arithmetic")
        result = subprocess.run(command, capture_output=True, text=True, timeout=120)
        observation = {
            "database": database.name, "case": label, "declared": declared,
            "baseline": baseline, "sql_sha256": hashlib.sha256(text.encode()).hexdigest(),
            "exit_code": result.returncode,
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
            "stderr_sha256": hashlib.sha256(result.stderr.encode()).hexdigest(),
        }
        report["observations"].append(observation)
        if wanted is None:
            if result.returncode != 2 or result.stdout or "declaration is unknown" not in result.stderr:
                raise AssertionError((observation, result.stderr))
            observation["refusal"] = "unknown widening"
            observation["rows"] = 0
        else:
            if result.returncode or result.stderr:
                raise AssertionError((observation, result.stderr, text))
            check_table(result.stdout, wanted)
            observation["rows"] = len(wanted)

    try:
        for name in INPUTS:
            database = args.directory / name
            getter = subprocess.run(
                ["java", "-Xmx256m", "-cp", str(args.jar),
                 str(ROOT / "docs/testing/TableDataOracle.java"), str(database), "ScalarSamples"],
                capture_output=True, text=True, timeout=120, check=True,
            )
            if getter.stderr:
                raise ValueError("Unexpected getter diagnostics")
            rows = mixed.native_rows(getter.stdout)
            report["getters"][name] = {
                "stdout_sha256": hashlib.sha256(getter.stdout.encode()).hexdigest(), "rows": len(rows),
            }
            for case in cases(rows):
                for reverse in [False, True]:
                    text = sql(case, reverse)
                    for declared in [False, True]:
                        for baseline in [False, True]:
                            run(database, case.name + ("-reversed" if reverse else ""), text,
                                declared, baseline, expected(case, declared, baseline, reverse))
            for expression, identity in [
                ("First([Octet])+CByte(1)", 1),
                ("First([Small])+CInt(1)", 2),
                ("-First([Small])", 1),
            ]:
                text = f"SELECT {expression} AS V FROM ScalarSamples WHERE ID={identity} UNION ALL SELECT CCur(4)"
                for baseline in [False, True]:
                    run(database, f"refuse-{identity}-{expression}", text, True, baseline)
            print(f"Checked {name}", flush=True)
        report["immutable_after"] = {name: mixed.digest(ROOT / name) for name in pins}
        if report["immutable_after"] != pins:
            raise ValueError("Original input, executable or tool changed")
        if any(mixed.digest(ROOT / name) != fingerprint for name, fingerprint in sources.items()):
            raise ValueError("Selected source changed")
        report["complete"] = True
    finally:
        report["cli_commands"] = len(report["observations"])
        report["refusals"] = sum("refusal" in item for item in report["observations"])
        report["compared_rows"] = sum(item.get("rows", 0) for item in report["observations"])
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Passed {report['cli_commands']} CLI observations", flush=True)


if __name__ == "__main__":
    main()
