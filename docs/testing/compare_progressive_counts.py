"""Replay native progressive Count(*) against original read-only getter records."""

import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path

from compare_decimal_seeks import JAR_SHA256, digest, getters, invoke, relative

ROOT = Path(__file__).resolve().parents[2]
COMPILED_REVISION = "905b885a"
CLI_SHA256 = "b51b570ab768359f734a6a3f37e88deb8d1b8f693d99e5bd83fa209a3402d0d4"
COMPILED_PATHS = [
    "crates/query/tablecraft-query/src/execution/progressive.rs",
    "crates/query/tablecraft-query/src/execution/progressive/admission.rs",
    "crates/query/tablecraft-query/src/execution/progressive/count.rs",
    "crates/query/tablecraft-query/src/execution/progressive/publication.rs",
    "crates/query/tablecraft-query/src/execution/progressive/rows.rs",
    "crates/query/tablecraft-query/src/execution/aggregate/count.rs",
    "crates/query/tablecraft-query/src/execution/context.rs",
    "crates/query/tablecraft-query/src/execution/execute.rs",
    "crates/backends/tablecraft-access/src/source/progressive.rs",
    "apps/tablecraft-cli/src/commands/query/run_args.rs",
    "apps/tablecraft-cli/src/commands/query/args.rs",
]
MODES = {
    "buffered": [],
    "stream1": ["--stream", "--batch-rows", "1"],
    "stream128": ["--stream", "--batch-rows", "128"],
}


def snapshot():
    revision = invoke(["git", "rev-parse", COMPILED_REVISION]).stdout.strip()
    records = {}
    for path in COMPILED_PATHS:
        result = invoke(["git", "show", f"{revision}:{path}"])
        if result.returncode:
            raise RuntimeError(result.stderr)
        records[path] = hashlib.sha256(result.stdout.encode()).hexdigest()
    return revision, records


class Replay:
    def __init__(self, cli):
        self.cli = cli
        self.controls = []
        self.compared_rows = 0

    def check(self, database, label, sql, mode, expected=None, error=None, extra=()):
        command = [
            self.cli, "--format", "tsv", "query", "run", sql,
            "--database", database, "--number-format", "invariant",
            *MODES[mode], *extra,
        ]
        actual = invoke(command)
        record = {
            "database": relative(database), "case": label, "mode": mode,
            "sql": sql, "extra": list(extra), "exit_code": actual.returncode,
            "stdout_sha256": hashlib.sha256(actual.stdout.encode()).hexdigest(),
        }
        if error is not None:
            if actual.returncode == 0 or actual.stdout or error not in actual.stderr.lower():
                raise RuntimeError(f"Wrong refusal for {label}: {actual}")
            record["refusal"] = error
            record["stderr"] = actual.stderr
        else:
            if actual.returncode or actual.stdout != expected:
                raise RuntimeError(
                    f"Count differs for {database.name}/{label}/{mode}: "
                    f"expected {expected!r}, got {actual.stdout!r}; {actual.stderr}"
                )
            record["rows"] = len(expected.splitlines()) - 1
            self.compared_rows += record["rows"]
        self.controls.append(record)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--inputs", required=True, nargs=4, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    if args.report.exists():
        raise RuntimeError("Accepted reports must be fresh")
    if digest(args.jar) != JAR_SHA256 or digest(args.cli) != CLI_SHA256:
        raise RuntimeError("Reviewed reference reader or frozen CLI differs")

    paths = [
        args.cli, args.jar, Path(__file__), *args.inputs,
        ROOT / "docs/testing/compare_decimal_seeks.py",
        ROOT / "docs/testing/DecimalIndexFixture.java",
        ROOT / "docs/testing/IndexRowsOracle.java",
    ]
    before = {relative(path): digest(path) for path in paths}
    revision, compiled = snapshot()
    replay = Replay(args.cli)
    models = {}
    for database in args.inputs:
        records, _, raw_hash = getters(args.jar, database, "Decimal4", 4)
        values = [value for _, value in records]
        counts = {
            "all": len(values),
            "null": sum(value is None for value in values),
            "nonnull": sum(value is not None for value in values),
            "positive": sum(value is not None and value > 0 for value in values),
            "negative": sum(value is not None and value < 0 for value in values),
            "equal": sum(value == Fraction(1, 10000) for value in values),
            "false": 0,
        }
        models[relative(database)] = {"getter_sha256": raw_hash, "counts": counts}
        filters = {
            "all": "True", "null": "[Value] IS NULL", "nonnull": "[Value] IS NOT NULL",
            "positive": "[Value]>0", "negative": "[Value]<0", "equal": "[Value]=P",
            "false": "False",
        }
        for mode in MODES:
            for label, predicate in filters.items():
                sql = f"SELECT Count(*) AS N FROM Decimal4 WHERE {predicate}"
                extra = []
                if label == "equal":
                    sql = "PARAMETERS P Decimal(28,4); " + sql
                    extra = ["--param", "P=decimal:0.0001"]
                replay.check(database, label, sql, mode, f"N\n{counts[label]}\n", extra=extra)
            replay.check(
                database, "aliases", "SELECT Count(*) AS N,N+1 AS NextValue FROM Decimal4",
                mode, f"N\tNextValue\n{len(values)}\t{len(values) + 1}\n",
            )
            replay.check(
                database, "mixed-arms",
                "SELECT Count(*) AS N FROM Decimal4 UNION ALL "
                "SELECT ID FROM Decimal4 WHERE ID=1 UNION ALL "
                "SELECT Count(*) FROM Decimal4 WHERE False",
                mode, f"N\n{len(values)}\n1\n0\n",
            )

        final_identity = max(identity for identity, _ in records)
        failures = [
            ("native-rows", "SELECT Count(*) FROM Decimal4", ["--native-rows", "1"], "resource"),
            ("query-steps", "SELECT Count(*) FROM Decimal4", ["--execution-steps", "1"], "resource"),
            ("output", "SELECT Count(*) FROM Decimal4", ["--output-bytes", "1"], "output"),
            ("eager-filter", "SELECT Count(*) FROM Decimal4 WHERE False And 1/0=0", [], "division"),
            (
                "late-filter",
                f"SELECT Count(*) FROM Decimal4 WHERE ID Mod (ID-{final_identity})=0",
                [], "division",
            ),
            ("aggregate-error", "SELECT 1/Count(*) FROM Decimal4 WHERE False", [], "division"),
        ]
        for label, sql, flags, error in failures:
            replay.check(database, label, sql, "stream1", error=error, extra=flags)

    after = {relative(path): digest(path) for path in paths}
    if before != after or snapshot() != (revision, compiled):
        raise RuntimeError("Immutable inputs or compiled revision changed during replay")
    report = {
        "date": "2026-10-25",
        "scope": (
            "Independent getter-model engineering Count(*) replay; "
            "no native Access qualification"
        ),
        "compiled_revision": revision, "compiled_revision_source_sha256": compiled,
        "compiled_source_note": "git show revision:path, not mutable live source; scoped inventory only",
        "immutable_sha256_before": before, "immutable_sha256_after": after,
        "models": models, "control_count": len(replay.controls),
        "successful_controls": sum("rows" in item for item in replay.controls),
        "refusal_controls": sum("refusal" in item for item in replay.controls),
        "compared_rows": replay.compared_rows, "observations": replay.controls,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in [
        "control_count", "successful_controls", "refusal_controls", "compared_rows",
    ]}))


if __name__ == "__main__":
    main()
