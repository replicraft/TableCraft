"""Check native integral GROUP/DISTINCT/UNION keys against inert getter models.

This original recipe reads existing registered fixtures only. TableCraft alone
executes SQL; subtype expectations describe its explicit profiles, not Access.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path

from compare_native_equijoins import INPUT_SHA256, JAR_SHA256, digest, getter_ids, invoke, relative

ROOT = Path(__file__).resolve().parents[2]
MODES = {
    "materialized": [],
    "declared": ["--declared-arithmetic"],
}
SOURCE_ROOTS = [
    "crates/query/tablecraft-query/src/execution/exact_keys",
    "crates/query/tablecraft-query/src/execution/grouping.rs",
    "crates/query/tablecraft-query/src/execution/retrieval.rs",
    "crates/query/tablecraft-query/src/execution/keys.rs",
    "crates/query/tablecraft-query/src/execution/union.rs",
    "crates/query/tablecraft-query/src/execution/union",
    "crates/query/tablecraft-query/src/plan/profiled_type.rs",
    "crates/backends/tablecraft-access/src/source.rs",
    "crates/backends/tablecraft-access/src/source/snapshot.rs",
    "crates/backends/tablecraft-access/src/source/projection.rs",
    "apps/tablecraft-cli/src/commands/query/retrieval.rs",
]


def compiled_sources(revision):
    listing = invoke(["git", "ls-tree", "-r", "--name-only", revision, "--", *SOURCE_ROOTS])
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
    if not any("/exact_keys/" in name for name in records):
        raise RuntimeError("Compiled revision lacks the exact-key implementation")
    return records


def unique(values):
    """Exact integer representatives, retaining their first getter encounter."""
    return list(dict.fromkeys(values))


def groups(values):
    counts = {}
    for value in values:
        counts[value] = counts.get(value, 0) + 1
    return list(counts.items())


def cases(identities, mode):
    values = identities["Decimal4"]
    residues = [value % 17 for value in values]
    yield (
        "native-field-groups",
        "SELECT ID AS K,Count(*) AS N FROM Decimal4 GROUP BY ID",
        ["K", "N"],
        groups(values),
    )
    yield (
        "native-field-distinct",
        "SELECT DISTINCT ID AS K FROM Decimal4",
        ["K"],
        [(value,) for value in unique(values)],
    )
    yield (
        "derived-repeated-groups",
        "SELECT K,Count(*) AS N FROM (SELECT ID Mod 17 AS K FROM Decimal4) Q GROUP BY K",
        ["K", "N"],
        groups(residues),
    )
    # Materialized unsuffixed 17 is Int64. The declared CLI preparation profile
    # captures it as Int16; the stored Int32 peer then selects Int32 Mod results.
    subtype = 20 if mode == "materialized" else 3
    yield (
        "computed-distinct-and-carrier",
        "SELECT DISTINCT ID Mod 17 AS K,VarType(ID Mod 17) AS CarrierCode FROM Decimal4",
        ["K", "CarrierCode"],
        [(value, subtype) for value in unique(residues)],
    )
    first = identities["Decimal0"]
    second = identities["Decimal4"]
    tail = [value for value in identities["Decimal28"] if value < 3]
    yield (
        "distinct-prefix-followed-by-all",
        "SELECT ID AS K FROM Decimal0 UNION SELECT ID FROM Decimal4 UNION ALL SELECT ID FROM Decimal28 WHERE ID<3",
        ["K"],
        [(value,) for value in [*unique([*first, *second]), *tail]],
    )
    yield (
        "all-prefix-followed-by-distinct",
        "SELECT ID AS K FROM Decimal0 UNION ALL SELECT ID FROM Decimal4 UNION SELECT ID FROM Decimal28 WHERE ID<3",
        ["K"],
        [(value,) for value in unique([*first, *second, *tail])],
    )
    yield (
        "union-all-population-groups",
        "SELECT ID AS K,Count(*) AS N FROM (SELECT ID FROM Decimal0 UNION ALL SELECT ID FROM Decimal4) Q GROUP BY ID",
        ["K", "N"],
        groups([*first, *second]),
    )
    yield (
        "nullable-field-key",
        "SELECT Nz(K,-1) AS K,Count(*) AS N FROM (SELECT ID AS K FROM Decimal4 UNION ALL SELECT NULL) Q GROUP BY K",
        ["K", "N"],
        [(-1 if value is None else value, count) for value, count in groups([*values, None])],
    )


class Replay:
    def __init__(self, cli):
        self.cli = cli
        self.observations = []
        self.compared_rows = 0
        self.compared_cells = 0

    def check(self, database, mode, label, sql, headings=None, rows=None, refusal=None, extra=()):
        if len(extra) % 2:
            raise RuntimeError("Quota overrides must be flag/value pairs")
        limits = {
            "--native-rows": "5000",
            "--execution-rows": "50000",
            "--execution-bytes": "134217728",
            "--execution-steps": "500000",
        }
        limits.update(zip(extra[::2], extra[1::2]))
        command = [
            self.cli, "--format", "tsv", "query", "run", sql,
            "--database", database, *MODES[mode],
        ]
        for flag, value in limits.items():
            command.extend([flag, value])
        result = invoke(command)
        record = {
            "profile": database.stem,
            "mode": mode,
            "case": label,
            "command": [relative(value) if isinstance(value, Path) else value for value in command],
            "exit_code": result.returncode,
            "stderr": result.stderr,
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
        }
        if refusal is not None:
            matches = result.returncode != 0 and not result.stdout and refusal in result.stderr.lower()
            record["expected_refusal"] = refusal
        else:
            expected = [[str(value) for value in row] for row in rows]
            actual = list(csv.reader(io.StringIO(result.stdout), delimiter="\t"))
            matches = result.returncode == 0 and not result.stderr and actual == [headings, *expected]
            record["expected_rows"] = len(rows)
            record["expected_rows_sha256"] = hashlib.sha256(json.dumps(rows).encode()).hexdigest()
            if matches:
                self.compared_rows += len(rows)
                self.compared_cells += sum(map(len, rows))
        record["matches"] = matches
        if not matches:
            record["stdout"] = result.stdout
        self.observations.append(record)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--inputs", type=Path, nargs=4, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if args.report.exists():
        raise RuntimeError("Refusing to overwrite an existing replay report")
    relative(args.report)
    if digest(args.cli) != args.cli_sha256 or digest(args.jar) != JAR_SHA256:
        raise RuntimeError("Frozen executable or reviewed reader differs")
    if {path.name for path in args.inputs} != set(INPUT_SHA256):
        raise RuntimeError("Supply exactly the four immutable registered originals")
    for path in args.inputs:
        if digest(path) != INPUT_SHA256[path.name]:
            raise RuntimeError("Registered original bytes differ")

    pinned = [
        args.cli, args.jar, Path(__file__),
        ROOT / "docs/testing/compare_native_equijoins.py",
        ROOT / "docs/testing/TableDataOracle.java",
        ROOT / "docs/testing/DecimalIndexFixture.java",
        *args.inputs,
    ]
    before = {relative(path): digest(path) for path in pinned}
    compiled = compiled_sources(args.revision)
    replay = Replay(args.cli)
    getter_observations = {}
    for database in args.inputs:
        identities, getter = getter_ids(args.jar, database)
        getter_observations[relative(database)] = getter
        for mode in MODES:
            for label, sql, headings, expected in cases(identities, mode):
                replay.check(database, mode, label, sql, headings, expected)
        sql = "SELECT DISTINCT ID AS K FROM Decimal4"
        for label, extra, refusal in [
            ("work-limit", ["--execution-steps", "1"], "resource"),
            ("native-row-limit", ["--native-rows", "1"], "resource"),
            ("output-limit", ["--output-bytes", "1"], "output"),
        ]:
            replay.check(database, "materialized", label, sql, refusal=refusal, extra=extra)

    after = {relative(path): digest(path) for path in pinned}
    if before != after or compiled_sources(args.revision) != compiled:
        raise RuntimeError("Pinned tools, originals or compiled revision changed")
    refusals = [record for record in replay.observations if "expected_refusal" in record]
    successes = [record for record in replay.observations if "expected_rows" in record]
    report = {
        "date": "2026-10-25",
        "scope": "Original inert native getters and exact integer key models; explicit TableCraft subtype profiles, no Access SQL oracle or timing claim",
        "compiled_revision": args.revision,
        "pins_before": before,
        "pins_after": after,
        "compiled_revision_source_sha256": compiled,
        "source_note": "Scoped git-show revision snapshots; mutable live production is not pinned",
        "getters": getter_observations,
        "controls": len(replay.observations),
        "success_controls": len(successes),
        "refusal_controls": len(refusals),
        "disagreements": sum(not record["matches"] for record in replay.observations),
        "compared_rows": replay.compared_rows,
        "compared_cells": replay.compared_cells,
        "observations": replay.observations,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as output:
        output.write(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in [
        "controls", "success_controls", "refusal_controls", "disagreements", "compared_rows", "compared_cells",
    ]}))
    if report["disagreements"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
