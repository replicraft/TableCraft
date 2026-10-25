"""Check original native Sum/Avg results against independent Fraction rounding."""

import argparse
import csv
from fractions import Fraction
import hashlib
import io
import json
from pathlib import Path

from compare_decimal_seeks import JAR_SHA256, digest, getters, invoke, relative

ROOT = Path(__file__).resolve().parents[2]
CLI_SHA256 = "c1e7c12722c31c31fbcdf47283b2ef0c92089d698b51f09cc200efda1cbc90c5"
REVISION = "e837d675137c8df82f296cfe9aab9f7b43ea6bb6"
MAX_COEFFICIENT = (1 << 96) - 1
MODES = {
    "buffered": [],
    "stream1": ["--stream", "--batch-rows", "1"],
    "stream128": ["--stream", "--batch-rows", "128"],
}
SOURCE_ROOTS = [
    "crates/query/tablecraft-query/src/execution/progressive",
    "crates/query/tablecraft-query/src/execution/progressive.rs",
    "crates/query/tablecraft-query/src/execution/aggregate",
    "crates/query/tablecraft-query/src/execution/context.rs",
    "crates/query/tablecraft-query/src/execution/execute.rs",
    "crates/backends/tablecraft-access/src/source/progressive.rs",
    "apps/tablecraft-cli/src/commands/query/run_args.rs",
    "apps/tablecraft-cli/src/commands/query/args.rs",
    "apps/tablecraft-cli/src/output.rs",
]


def compiled_sources():
    listing = invoke(["git", "ls-tree", "-r", "--name-only", REVISION, "--", *SOURCE_ROOTS])
    if listing.returncode:
        raise RuntimeError(listing.stderr)
    records = {}
    for path in listing.stdout.splitlines():
        source = invoke(["git", "show", f"{REVISION}:{path}"])
        if source.returncode:
            raise RuntimeError(source.stderr)
        records[path] = hashlib.sha256(source.stdout.encode()).hexdigest()
    return records


def half_even(value):
    """Nearest integer by exact distance, resolving halfway points to even."""
    magnitude, remainder = divmod(abs(value.numerator), value.denominator)
    twice = 2 * remainder
    if twice > value.denominator or twice == value.denominator and magnitude % 2:
        magnitude += 1
    return -magnitude if value < 0 else magnitude


def decimal_result(value):
    """Highest-resolution scale-0..28 Decimal with a fitting 96-bit coefficient."""
    for scale in range(28, -1, -1):
        coefficient = half_even(value * 10 ** scale)
        if abs(coefficient) <= MAX_COEFFICIENT:
            return Fraction(coefficient, 10 ** scale)
    raise OverflowError("Exact final total lies outside the Decimal result envelope")


def population_model(records):
    present = [value for _, value in records if value is not None]
    if not present:
        return [len(records), 0, None, None, 1, 1]
    total = sum(present, Fraction(0))
    return [
        len(records), len(present), decimal_result(total),
        decimal_result(total / len(present)), 14, 14,
    ]


def show_row(row):
    return [None if value is None else str(value) for value in row]


class Replay:
    def __init__(self, cli):
        self.cli = cli
        self.controls = []
        self.compared_rows = 0
        self.compared_cells = 0

    def check(self, database, label, sql, mode, headings=None, rows=None, error=None, extra=()):
        command = [
            self.cli, "--format", "tsv", "query", "run", sql,
            "--database", database, "--number-format", "invariant",
            *MODES[mode], *extra,
        ]
        result = invoke(command)
        record = {
            "database": relative(database), "case": label, "mode": mode,
            "sql": sql, "extra": list(extra), "exit_code": result.returncode,
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
        }
        if error is not None:
            if result.returncode == 0 or result.stdout or error not in result.stderr.lower():
                raise RuntimeError(f"Wrong {error} refusal for {label}: {result}")
            record["refusal"] = error
            record["stderr"] = result.stderr
        else:
            parsed = list(csv.reader(io.StringIO(result.stdout), delimiter="\t"))
            if result.returncode or not parsed or parsed[0] != headings:
                raise RuntimeError(f"Wrong headings/status for {label}: {result}")
            actual = [
                [None if cell == "NULL" else Fraction(cell) for cell in row]
                for row in parsed[1:]
            ]
            if actual != rows:
                raise RuntimeError(f"Numeric model differs for {label}: {actual} != {rows}")
            record["rows"] = len(rows)
            record["expected_fraction_cells"] = [show_row(row) for row in rows]
            self.compared_rows += len(rows)
            self.compared_cells += sum(len(row) for row in rows)
        self.controls.append(record)

    def statistics(self, database, label, table, records, predicate="True", extra=()):
        sql = (
            "SELECT Count(*) AS N,Count([Value]) AS C,Sum([Value]) AS S,"
            "Avg([Value]) AS A,VarType(Sum([Value])) AS ST,"
            f"VarType(Avg([Value])) AS AT FROM [{table}] WHERE {predicate}"
        )
        if "P=" in " ".join(extra):
            sql = "PARAMETERS P Decimal(28,4); " + sql
        try:
            expected = population_model(records)
            error = None
        except OverflowError:
            expected = None
            error = "overflow"
        for mode in MODES:
            self.check(
                database, label, sql, mode, ["N", "C", "S", "A", "ST", "AT"],
                [expected] if expected is not None else None, error=error, extra=extra,
            )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--inputs", required=True, nargs=4, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    if args.report.exists():
        raise RuntimeError("Accepted reports must be fresh")
    if digest(args.cli) != CLI_SHA256 or digest(args.jar) != JAR_SHA256:
        raise RuntimeError("Frozen executable or reviewed reference reader differs")

    pinned = [
        args.cli, args.jar, Path(__file__), *args.inputs,
        ROOT / "docs/testing/compare_decimal_seeks.py",
        ROOT / "docs/testing/DecimalIndexFixture.java",
        ROOT / "docs/testing/IndexRowsOracle.java",
    ]
    before = {relative(path): digest(path) for path in pinned}
    compiled = compiled_sources()
    replay = Replay(args.cli)
    models = {}
    for database in args.inputs:
        for scale in [0, 4, 28]:
            table = f"Decimal{scale}"
            records, _, getter_hash = getters(args.jar, database, table, scale)
            models[f"{database.name}/{table}"] = {
                "getter_sha256": getter_hash,
                "count": len(records),
                "present": sum(value is not None for _, value in records),
                "exact_sum": str(sum((value for _, value in records if value is not None), Fraction(0))),
            }
            replay.statistics(database, f"scale{scale}-all", table, records)
            for sign, predicate, select in [
                ("positive", "[Value]>0", lambda value: value > 0),
                ("negative", "[Value]<0", lambda value: value < 0),
            ]:
                selected = [
                    (identity, value)
                    for identity, value in records
                    if value is not None and select(value)
                ]
                exact_total = sum((value for _, value in selected), Fraction(0))
                try:
                    bounded_total = str(decimal_result(exact_total))
                except OverflowError:
                    bounded_total = "overflow"
                models[f"{database.name}/{table}/{sign}"] = {
                    "source": f"{database.name}/{table}",
                    "count": len(selected),
                    "exact_sum": str(exact_total),
                    "exact_mean": str(exact_total / len(selected)),
                    "expected_sum": bounded_total,
                    "expected_mean": str(decimal_result(exact_total / len(selected))),
                }
                replay.statistics(database, f"scale{scale}-{sign}", table, selected, predicate)
                if scale == 0:
                    values = [value for _, value in selected]
                    mean = decimal_result(sum(values, Fraction(0)) / len(values))
                    for mode in MODES:
                        replay.check(
                            database, f"overflow-sum-but-valid-mean-{sign}",
                            f"SELECT Avg([Value]) AS A FROM [{table}] WHERE {predicate}",
                            mode, ["A"], [[mean]],
                        )
            if scale != 4:
                continue
            replay.statistics(database, "all-null", table,
                              [(identity, value) for identity, value in records if value is None],
                              "[Value] IS NULL")
            replay.statistics(database, "filtered-empty", table, [], "False")
            point = Fraction(1, 10000)
            matches = [(identity, value) for identity, value in records if value == point]
            replay.statistics(database, "typed-point", table, matches, "[Value]=P",
                              ["--param", "P=decimal:0.0001"])
            replay.statistics(database, "declared-policy", table, records,
                              extra=["--declared-arithmetic"])

            empty = invoke([
                "java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/IndexRowsOracle.java", database, "DecimalEmpty4", "EmptyKey", "Value",
            ])
            if empty.returncode or empty.stdout:
                raise RuntimeError("Original public getter did not confirm empty table")
            models[f"{database.name}/DecimalEmpty4"] = {
                "getter_sha256": hashlib.sha256(empty.stdout.encode()).hexdigest(), "count": 0,
            }
            replay.statistics(database, "native-empty-table", "DecimalEmpty4", [])
            total = population_model(records)[2]
            identities = [identity for identity, _ in records]
            zero_population = [(identity, value) for identity, value in records if value == 0]
            zero_model = population_model(zero_population)
            first_present = sum(identity == 1 and value is not None for identity, value in records)
            for mode in MODES:
                replay.check(
                    database, "integral-controls",
                    "SELECT Sum(ID) AS S,Avg(ID) AS A,VarType(Sum(ID)) AS ST,VarType(Avg(ID)) AS AT FROM Decimal4",
                    mode, ["S", "A", "ST", "AT"],
                    [[sum(identities), Fraction(sum(identities), len(identities)), 20, 5]],
                )
                replay.check(
                    database, "prior-alias", "SELECT Sum([Value]) AS S,S+0 AS Echo FROM Decimal4",
                    mode, ["S", "Echo"], [[total, total]],
                )
                replay.check(
                    database, "mixed-arms",
                    "SELECT Sum([Value]) AS N FROM Decimal4 WHERE [Value]=CDec(0) UNION ALL "
                    "SELECT Avg([Value]) FROM Decimal4 WHERE [Value]=CDec(0) UNION ALL "
                    "SELECT Count([Value]) FROM Decimal4 WHERE ID=1",
                    mode, ["N"], [[zero_model[2]], [zero_model[3]], [first_present]],
                )
            final_identity = max(identities)
            aggregate_sql = "SELECT Sum([Value]),Avg([Value]) FROM Decimal4"
            for label, sql, extra, error in [
                (
                    "native-limit", aggregate_sql,
                    ["--native-rows", "1"], "resource",
                ),
                (
                    "query-limit", aggregate_sql,
                    ["--execution-steps", "1"], "resource",
                ),
                (
                    "output-limit", aggregate_sql,
                    ["--output-bytes", "1"], "output",
                ),
                (
                    "late-operand",
                    f"SELECT Sum(ID Mod (ID-{final_identity})),Avg([Value]) FROM Decimal4",
                    [], "division",
                ),
            ]:
                replay.check(database, label, sql, "stream1", error=error, extra=extra)

    after = {relative(path): digest(path) for path in pinned}
    if before != after or compiled_sources() != compiled:
        raise RuntimeError("Immutable replay inputs or compiled revision changed")
    report = {
        "date": "2026-10-25", "compiled_revision": REVISION,
        "scope": "Independent native getter/Fraction Sum/Avg engineering evidence; no Access qualification",
        "immutable_sha256_before": before, "immutable_sha256_after": after,
        "compiled_revision_source_sha256": compiled,
        "source_note": "Scoped git-show revision snapshots; mutable live production is not pinned",
        "models": models, "control_count": len(replay.controls),
        "successful_controls": sum("rows" in record for record in replay.controls),
        "refusal_controls": sum("refusal" in record for record in replay.controls),
        "compared_rows": replay.compared_rows, "compared_cells": replay.compared_cells,
        "observations": replay.controls,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in [
        "control_count", "successful_controls", "refusal_controls", "compared_rows", "compared_cells",
    ]}))


if __name__ == "__main__":
    main()
