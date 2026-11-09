"""Check native progressive extrema against inert getters and exact models.

Only TableCraft executes SQL. The public Java reader observes original rows;
Fraction, integer and ordinal models supply expectations without executing
reference SQL or claiming native Access evaluation equivalence.
"""

import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path

from compare_decimal_seeks import JAR_SHA256, digest, getters, invoke, relative
from compare_progressive_numeric import MODES, Replay

ROOT = Path(__file__).resolve().parents[2]
SOURCE_ROOTS = [
    "crates/query/tablecraft-query/src/execution/aggregate.rs",
    "crates/query/tablecraft-query/src/execution/aggregate/extrema.rs",
    "crates/query/tablecraft-query/src/execution/aggregate/extrema/state.rs",
    "crates/query/tablecraft-query/src/execution/aggregate/extrema/policy.rs",
    "crates/query/tablecraft-query/src/execution/aggregate/accumulator.rs",
    "crates/query/tablecraft-query/src/execution/context.rs",
    "crates/query/tablecraft-query/src/execution/variants.rs",
    "crates/query/tablecraft-query/src/execution/progressive.rs",
    "crates/query/tablecraft-query/src/execution/progressive/admission.rs",
    "crates/query/tablecraft-query/src/execution/progressive/aggregates.rs",
    "crates/query/tablecraft-query/src/execution/progressive/plan.rs",
    "crates/query/tablecraft-query/src/execution/progressive/rows.rs",
    "crates/backends/tablecraft-access/src/source/progressive.rs",
    "apps/tablecraft-cli/src/commands/query/run_args.rs",
    "apps/tablecraft-cli/src/commands/query/retrieval.rs",
    "apps/tablecraft-cli/src/output.rs",
]


def compiled_sources(revision):
    listing = invoke([
        "git", "ls-tree", "-r", "--name-only", revision, "--", *SOURCE_ROOTS,
    ])
    if listing.returncode:
        raise RuntimeError(listing.stderr)

    records = {}
    for name in listing.stdout.splitlines():
        if not name.endswith(".rs"):
            continue
        source = invoke(["git", "show", f"{revision}:{name}"])
        if source.returncode:
            raise RuntimeError(source.stderr)
        records[name] = hashlib.sha256(source.stdout.encode()).hexdigest()

    if not any(name.endswith("extrema/state.rs") for name in records):
        raise RuntimeError("Compiled revision lacks the shared extrema state")
    return records


def population(records):
    present = [value for _, value in records if value is not None]
    if not present:
        return [None, None, 1, 1, len(records), 0]
    return [min(present), max(present), 14, 14, len(records), len(present)]


def statistics(replay, database, table, label, records, predicate="True", extra=()):
    sql = (
        "SELECT Min([Value]) AS Low,Max([Value]) AS High,"
        "VarType(Min([Value])) AS LT,VarType(Max([Value])) AS HT,"
        f"Count(*) AS N,Count([Value]) AS C FROM [{table}] WHERE {predicate}"
    )
    if extra:
        sql = "PARAMETERS P Decimal(28,4); " + sql
    expected = population(records)
    for mode in MODES:
        replay.check(
            database, label, sql, mode,
            ["Low", "High", "LT", "HT", "N", "C"], [expected], extra=extra,
        )


def additional_cases(replay, database, records):
    identities = [identity for identity, _ in records]
    low = min(identities)
    high = max(identities)
    text_low = int(min(str(identity) for identity in identities))
    text_high = int(max(str(identity) for identity in identities))
    cases = [
        (
            "integral-and-alias",
            "SELECT Min(ID) AS Low,Max(ID) AS High,Low+0 AS Echo,"
            "VarType(Min(ID)) AS T FROM Decimal4",
            ["Low", "High", "Echo", "T"], [[low, high, low, 3]], (),
        ),
        (
            "ordinal-text",
            "SELECT CLng(Min(CStr(ID))) AS Low,CLng(Max(CStr(ID))) AS High "
            "FROM Decimal4",
            ["Low", "High"], [[text_low, text_high]], ["--collation", "ordinal"],
        ),
        (
            "boolean-order",
            "SELECT CLng(Min(CBool(ID))) AS Low,CLng(Max(CBool(ID))) AS High "
            "FROM Decimal4",
            ["Low", "High"], [[-1, 0]], (),
        ),
        (
            "standard-error-code-order",
            "SELECT CLng(Min(CVErr(ID))) AS Low,CLng(Max(CVErr(ID))) AS High "
            "FROM Decimal4",
            ["Low", "High"], [[low, high]], (),
        ),
        (
            "singleton-raw-observers",
            "SELECT LenB(Min(ChrW(55296))) AS U,LenB(Max(ChrB(65))) AS B "
            "FROM Decimal4 WHERE ID=0",
            ["U", "B"], [[2, 1]], (),
        ),
        (
            "mixed-aggregate-arms",
            "SELECT Min([Value]) AS V FROM Decimal4 UNION ALL "
            "SELECT Max([Value]) FROM Decimal4 UNION ALL "
            "SELECT Count(*) FROM Decimal4 WHERE False",
            ["V"], [[population(records)[0]], [population(records)[1]], [0]], (),
        ),
    ]
    for label, sql, headings, rows, extra in cases:
        for mode in MODES:
            for declared in [False, True]:
                policies = list(extra)
                if declared:
                    policies.append("--declared-arithmetic")
                replay.check(
                    database, f"{label}-declared-{declared}", sql, mode,
                    headings, rows, extra=policies,
                )

    for label, sql, extra, error in [
        (
            "late-operand",
            f"SELECT Min(ID Mod (ID-{high})),Max(ID) FROM Decimal4", [], "division",
        ),
        (
            "native-budget", "SELECT Min(ID),Max(ID) FROM Decimal4",
            ["--native-rows", "1"], "resource",
        ),
        (
            "query-budget", "SELECT Min(ID),Max(ID) FROM Decimal4",
            ["--execution-steps", "1"], "resource",
        ),
        (
            "output-budget", "SELECT Min(ID),Max(ID) FROM Decimal4",
            ["--output-bytes", "1"], "output",
        ),
    ]:
        replay.check(database, label, sql, "stream1", error=error, extra=extra)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--inputs", required=True, nargs=4, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    if args.report.exists():
        raise RuntimeError("Accepted reports must be fresh")
    if digest(args.cli) != args.cli_sha256 or digest(args.jar) != JAR_SHA256:
        raise RuntimeError("Frozen executable or reviewed independent reader differs")

    revision = invoke(["git", "rev-parse", "--verify", f"{args.revision}^{{commit}}"])
    if revision.returncode:
        raise RuntimeError(revision.stderr)
    revision = revision.stdout.strip()
    pinned = [args.cli, args.jar, Path(__file__), *args.inputs]
    pinned.extend(ROOT / name for name in [
        "docs/testing/compare_progressive_numeric.py",
        "docs/testing/compare_decimal_seeks.py",
        "docs/testing/DecimalIndexFixture.java",
        "docs/testing/IndexRowsOracle.java",
    ])
    before = {relative(path): digest(path) for path in pinned}
    compiled = compiled_sources(revision)
    replay = Replay(args.cli)
    models = {}
    for database in args.inputs:
        for scale in [0, 4, 28]:
            table = f"Decimal{scale}"
            records, _, getter_hash = getters(args.jar, database, table, scale)
            models[f"{database.name}/{table}"] = {
                "getter_sha256": getter_hash,
                "expected_fraction_cells": [str(value) for value in population(records)],
            }
            statistics(replay, database, table, "all", records)
            for label, predicate, select in [
                ("positive", "[Value]>0", lambda value: value is not None and value > 0),
                ("negative", "[Value]<0", lambda value: value is not None and value < 0),
                ("all-null", "[Value] IS NULL", lambda value: value is None),
                ("filtered-empty", "False", lambda value: False),
            ]:
                selected = [(identity, value) for identity, value in records if select(value)]
                statistics(replay, database, table, label, selected, predicate)
            if scale == 4:
                point = Fraction(1, 10000)
                selected = [(identity, value) for identity, value in records if value == point]
                statistics(
                    replay, database, table, "typed-point", selected, "[Value]=P",
                    ["--param", "P=decimal:0.0001"],
                )
                empty = invoke([
                    "java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    "docs/testing/IndexRowsOracle.java", database,
                    "DecimalEmpty4", "EmptyKey", "Value",
                ])
                if empty.returncode or empty.stdout:
                    raise RuntimeError("Public getter did not confirm native empty table")
                models[f"{database.name}/DecimalEmpty4"] = {
                    "getter_sha256": hashlib.sha256(empty.stdout.encode()).hexdigest(),
                    "count": 0,
                }
                statistics(replay, database, "DecimalEmpty4", "native-empty", [])
                additional_cases(replay, database, records)

    after = {relative(path): digest(path) for path in pinned}
    if before != after or compiled_sources(revision) != compiled:
        raise RuntimeError("Immutable replay inputs or compiled source snapshots changed")
    report = {
        "date": "2026-10-25", "compiled_revision": revision,
        "scope": "Original native getter/exact-model extrema evidence; no Access evaluation claim",
        "immutable_sha256_before": before, "immutable_sha256_after": after,
        "compiled_revision_source_sha256": compiled,
        "source_note": "Selected commit source snapshots, not a compilation manifest; mutable production is not pinned",
        "models": models, "control_count": len(replay.controls),
        "compared_rows": replay.compared_rows, "compared_cells": replay.compared_cells,
        "observations": replay.controls,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in [
        "control_count", "compared_rows", "compared_cells",
    ]}))


if __name__ == "__main__":
    main()
