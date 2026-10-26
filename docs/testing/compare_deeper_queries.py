"""Replay compound literal and native-field queries at default and explicit depth.

Only TableCraft executes SQL. Exact Fraction/IEEE models and independent stored
getters supply expected values; this does not qualify native Access semantics.
"""

import argparse
import csv
import hashlib
import io
import itertools
import json
from pathlib import Path
import re
import subprocess

import compare_fixed_binary_keys as mixed
import compare_numeric_keys as model
from compare_declared_numeric_text import INPUTS, JAR_SHA256

ROOT = Path(__file__).resolve().parents[2]
MAX_COMMANDS = 2000
RETAINED_WRAPPERS = 6
SOURCE_ROOTS = [
    "crates/query/tablecraft-query/src/binding",
    "crates/query/tablecraft-query/src/discovery",
    "crates/query/tablecraft-query/src/plan/text_policy.rs",
    "crates/query/tablecraft-query/src/execution/options.rs",
    "crates/query/tablecraft-query/src/execution/execute.rs",
    "apps/tablecraft-cli/src/commands/query/binding_depth.rs",
    "apps/tablecraft-cli/src/commands/query/run_args.rs",
]


def queries(case, order):
    arms = []
    # An explicit floating spelling avoids the independent integer-literal
    # envelope when constructing Double values beyond the signed LongLong range.
    def floating_literal(match):
        value = match.group(1)
        if abs(int(value)) > 9223372036854775807:
            return f"CDbl({value}.0)"
        return match.group(0)

    expressions = tuple(
        re.sub(r"CDbl\((-?\d+)\)", floating_literal, expression)
        for expression in case.family.expressions
    )
    for index in order:
        expression = expressions[index - 1]
        other = next(value for value in expressions if value != expression)
        selected = f"IIf(CBool(True),{expression},{other})"
        for _ in range(RETAINED_WRAPPERS):
            selected = f"CVar({selected})"
        selected += " AS K"
        arm = f"SELECT CInt({index}) AS Id,{selected}"
        if case.native_row is not None:
            arm += f" FROM ScalarSamples S WHERE S.ID={case.native_row}"
        arms.append(arm)

    source = "(" + " UNION ALL ".join(arms) + ") Numbers"
    group = f"SELECT K,Count(*) AS N FROM {source} GROUP BY K"
    extrema = f"SELECT Min(K) AS L,Max(K) AS H FROM {source}"
    return {
        "group": f"SELECT VarType(K) AS T,K AS V,N FROM ({group}) G",
        "order": f"SELECT Id,VarType(K) AS T,K AS V FROM {source} ORDER BY K",
        "extrema": f"SELECT VarType(L) AS LT,L AS L,VarType(H) AS HT,H AS H FROM ({extrema}) E",
    }


def source_pins(revision):
    listing = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", revision, "--", *SOURCE_ROOTS],
        check=True, capture_output=True, text=True,
    )
    result = {}
    for name in listing.stdout.splitlines():
        if not name.endswith(".rs"):
            continue
        content = subprocess.run(
            ["git", "show", f"{revision}:{name}"], check=True, capture_output=True,
        ).stdout
        expected = hashlib.sha256(content).hexdigest()
        if mixed.digest(ROOT / name) != expected:
            raise ValueError("Selected source differs from committed revision")
        result[name] = expected
    if not result:
        raise ValueError("Missing selected source snapshots")
    return result


class Replay:
    def __init__(self, args):
        self.args = args
        self.observations = []

    def run(self, case, order, declared, consumer, sql, database, explicit):
        if len(self.observations) >= MAX_COMMANDS:
            raise ValueError("CLI command ceiling exceeded")
        command = [str(self.args.cli), "--format", "tsv", "query", "run", sql,
                   "--number-format", "invariant", "--collation", "ordinal"]
        if declared:
            command.append("--declared-arithmetic")
        if database is not None:
            command.extend(["--database", str(database)])
        if explicit:
            command.extend(["--binding-depth", "32"])
        result = subprocess.run(command, capture_output=True, text=True,
                                encoding="utf-8", timeout=120)
        observation = {
            "case": case.family.name,
            "order": list(order),
            "declared": declared,
            "consumer": consumer,
            "database": database.name if database else None,
            "binding_depth": 32 if explicit else 16,
            "sql_sha256": hashlib.sha256(sql.encode()).hexdigest(),
            "exit_code": result.returncode,
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
            "stderr_sha256": hashlib.sha256(result.stderr.encode()).hexdigest(),
        }
        self.observations.append(observation)
        if not explicit and result.returncode:
            if result.returncode != 2 or result.stdout or "structural limit" not in result.stderr:
                raise AssertionError((observation, result.stderr))
            observation.update(refusal="default structural limit", rows=0, cells=0)
            return
        if result.returncode or result.stderr:
            raise AssertionError((observation, result.stderr, sql))
        table = list(csv.reader(io.StringIO(result.stdout), delimiter="\t", strict=True))
        model.validate(case.family, consumer, mixed.transport_table(table, consumer))
        observation.update(rows=len(table) - 1, cells=sum(len(row) for row in table[1:]))

    def case(self, case, database=None):
        for order, declared in itertools.product(model.ORDERS, [False, True]):
            for consumer, sql in queries(case, order).items():
                self.run(case, order, declared, consumer, sql, database, explicit=True)
                if order == model.ORDERS[0] and not declared:
                    self.run(case, order, declared, consumer, sql, database, explicit=False)
        print(f"Checked {database.name if database else 'literals'}: {case.family.name}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory", "report"]:
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    if args.report.exists():
        raise ValueError("Report must be fresh")
    if mixed.digest(args.cli) != args.cli_sha256 or mixed.digest(args.jar) != JAR_SHA256:
        raise ValueError("Executable or reference fingerprint differs")
    for name, expected in INPUTS.items():
        if mixed.digest(args.directory / name) != expected:
            raise ValueError("Original native input fingerprint differs")
    revision = subprocess.run(
        ["git", "rev-parse", "--verify", f"{args.revision}^{{commit}}"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
    tools = [args.cli, args.jar, Path(__file__),
             ROOT / "docs/testing/compare_fixed_binary_keys.py",
             ROOT / "docs/testing/compare_numeric_keys.py",
             ROOT / "docs/testing/compare_declared_numeric_text.py",
             ROOT / "docs/testing/TableDataOracle.java",
             *(args.directory / name for name in INPUTS)]
    pins = {str(path.resolve().relative_to(ROOT)): mixed.digest(path) for path in tools}
    sources = source_pins(revision)
    replay = Replay(args)
    report = {
        "date": "2026-10-25", "revision": revision, "scope": __doc__,
        "model_goldens": model.model_controls(),
        "immutable_before": pins, "selected_source_sha256": sources,
        "getter_records": {}, "observations": replay.observations, "complete": False,
    }
    try:
        for case in mixed.fixed_cases():
            replay.case(case)
        for name in INPUTS:
            database = args.directory / name
            getter = subprocess.run(
                ["java", "-Xmx256m", "-cp", str(args.jar),
                 str(ROOT / "docs/testing/TableDataOracle.java"), str(database), "ScalarSamples"],
                capture_output=True, text=True, encoding="utf-8", timeout=120, check=True,
            )
            if getter.stderr:
                raise ValueError("Unexpected getter diagnostics")
            rows = mixed.native_rows(getter.stdout)
            report["getter_records"][name] = {
                "rows": len(rows),
                "stdout_sha256": hashlib.sha256(getter.stdout.encode()).hexdigest(),
            }
            for case in mixed.native_cases(rows):
                replay.case(case, database)
        report["immutable_after"] = {path: mixed.digest(ROOT / path) for path in pins}
        if report["immutable_after"] != pins or source_pins(revision) != sources:
            raise ValueError("Pinned input, executable, tool or source changed")
        if not any("refusal" in item for item in replay.observations):
            raise AssertionError("Replay must exercise actual default-depth refusals")
        report["complete"] = True
    finally:
        report["cli_commands"] = len(replay.observations)
        report["explicit_commands"] = sum(item["binding_depth"] == 32 for item in replay.observations)
        report["default_refusals"] = sum("refusal" in item for item in replay.observations)
        report["compared_rows"] = sum(item.get("rows", 0) for item in replay.observations)
        report["compared_cells"] = sum(item.get("cells", 0) for item in replay.observations)
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Passed {report['cli_commands']} CLI observations", flush=True)


if __name__ == "__main__":
    main()
