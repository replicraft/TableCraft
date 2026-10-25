"""Replay sparse native ID joins against inert sequential Jackcess getter rows.

Only TableCraft executes SQL. This measures correctness/resource admission, not
speed or native Microsoft Access SQL compatibility. No fixture is generated.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
INPUT_SHA256 = {
    "decimal-seeks-V2000-20261025.mdb": "0ec45083fd4aea8cde561ecac5a6bec472a019d9a62249914120c9ad6fc1782a",
    "decimal-seeks-V2007-20261025.accdb": "7d6bf4d00e6da1a674e2ac8dbc5fea9e6ab84a26fc09ae7e98010c22cc6ffc21",
    "decimal-seeks-V2016-20261025.accdb": "be4d1f2d6bc9005263ff1ae90f442897f2d63b7d02f7e2b2bfcbfc00a26dc63c",
    "decimal-seeks-V2019-20261025.accdb": "555d1544d87d54d82648a36a2e46148dc1c9911d07b2070909dfd3710799cb37",
}
TABLES = ["Decimal0", "Decimal4", "Decimal28"]
TIMEOUT_SECONDS = 120
SOURCE_ROOTS = [
    "crates/query/tablecraft-query/src/execution/relation.rs",
    "crates/query/tablecraft-query/src/execution/relation",
    "crates/query/tablecraft-query/src/execution/retrieval.rs",
    "crates/query/tablecraft-query/src/execution/cell.rs",
    "crates/query/tablecraft-query/src/execution/collation.rs",
    "crates/query/tablecraft-query/src/execution/collation",
    "crates/backends/tablecraft-access/src/source.rs",
    "crates/backends/tablecraft-access/src/source/snapshot.rs",
    "crates/backends/tablecraft-access/src/source/projection.rs",
    "apps/tablecraft-cli/src/commands/query/retrieval.rs",
    "apps/tablecraft-cli/src/commands/query/native_target.rs",
]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def invoke(arguments):
    return subprocess.run(
        [str(argument) for argument in arguments],
        text=True,
        capture_output=True,
        timeout=TIMEOUT_SECONDS,
    )


def command_record(arguments):
    return [relative(value) if isinstance(value, Path) else value for value in arguments]


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
    if not records:
        raise RuntimeError("Compiled revision has no scoped source records")
    return records


def getter_ids(jar, database):
    command = [
        "java", "-Dstdout.encoding=UTF-8", "-cp", jar,
        ROOT / "docs/testing/TableDataOracle.java", database, *TABLES,
    ]
    result = invoke(command)
    if result.returncode or result.stderr:
        raise RuntimeError(f"Independent getter failed: {result.stderr}")
    identities = {name: [] for name in TABLES}
    for line in result.stdout.splitlines():
        table, identity, value = line.split("\t")
        if table not in identities or not identity.startswith("I32:"):
            raise RuntimeError("Unexpected ordinary getter carrier or table")
        if value != "N" and not value.startswith("Q"):
            raise RuntimeError("Unexpected original Decimal carrier")
        identities[table].append(int(identity[4:]))
    for values in identities.values():
        if len(values) != 1000 or len(set(values)) != 1000:
            raise RuntimeError("Original sparse ID corpus has unexpected cardinality")
    return identities, {
        "command": command_record(command),
        "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
        "row_count": sum(map(len, identities.values())),
        "ids_in_getter_encounter_order": identities,
    }


def inner(left, right):
    """Independent integer predicate in left-major/right-minor encounter order."""
    return [(a, b) for a in left for b in right if a == b]


def left_outer(left, right):
    rows = []
    for a in left:
        matches = [(a, b) for b in right if a == b]
        rows.extend(matches if matches else [(a, -1)])
    return rows


def right_outer(left, right):
    rows = inner(left, right)
    matched = {b for _, b in rows}
    rows.extend((-1, b) for b in right if b not in matched)
    return rows


def cases(identities):
    values = identities["Decimal4"]
    pairs = inner(values, values)
    yield (
        "self-equality",
        "SELECT A.ID AS LeftID,B.ID AS RightID FROM Decimal4 A INNER JOIN Decimal4 B ON A.ID=B.ID",
        pairs,
    )
    yield (
        "swapped-operands",
        "SELECT A.ID AS LeftID,B.ID AS RightID FROM Decimal4 A INNER JOIN Decimal4 B ON B.ID=A.ID",
        pairs,
    )
    yield (
        "post-join-filter",
        "SELECT A.ID AS LeftID,B.ID AS RightID FROM Decimal4 A INNER JOIN Decimal4 B ON A.ID=B.ID WHERE A.ID<3",
        [(a, b) for a, b in pairs if a < 3],
    )
    left = identities["Decimal0"]
    right = [identity for identity in identities["Decimal28"] if identity < 3]
    yield (
        "left-unmatched",
        "SELECT A.ID AS LeftID,Nz(B.ID,-1) AS RightID FROM Decimal0 A LEFT JOIN (SELECT ID FROM Decimal28 WHERE ID<3) B ON A.ID=B.ID",
        left_outer(left, right),
    )
    left = [identity for identity in identities["Decimal0"] if identity < 3]
    right = identities["Decimal28"]
    yield (
        "right-unmatched",
        "SELECT Nz(A.ID,-1) AS LeftID,B.ID AS RightID FROM (SELECT ID FROM Decimal0 WHERE ID<3) A RIGHT JOIN Decimal28 B ON A.ID=B.ID",
        right_outer(left, right),
    )


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
        raise RuntimeError("Frozen executable or reviewed independent reader differs")
    if {path.name for path in args.inputs} != set(INPUT_SHA256):
        raise RuntimeError("Supply exactly the four registered immutable originals")
    for path in args.inputs:
        if digest(path) != INPUT_SHA256[path.name]:
            raise RuntimeError("Original native input differs from registered hash")

    pinned = [
        args.cli, args.jar, Path(__file__),
        ROOT / "docs/testing/TableDataOracle.java",
        ROOT / "docs/testing/DecimalIndexFixture.java",
        *args.inputs,
    ]
    before = {relative(path): digest(path) for path in pinned}
    compiled = compiled_sources(args.revision)
    getter_observations = {}
    observations = []
    compared_rows = 0
    compared_cells = 0
    for database in args.inputs:
        identities, getter = getter_ids(args.jar, database)
        getter_observations[relative(database)] = getter
        for label, sql, expected in cases(identities):
            command = [
                args.cli, "--format", "tsv", "query", "run", sql,
                "--database", database,
                "--native-rows", "5000",
                "--execution-rows", "10000",
                "--execution-bytes", "268435456",
                "--execution-steps", "100000000",
            ]
            result = invoke(command)
            expected_rows = [[str(a), str(b)] for a, b in expected]
            actual = list(csv.reader(io.StringIO(result.stdout), delimiter="\t"))
            matches = (
                result.returncode == 0
                and not result.stderr
                and actual == [["LeftID", "RightID"], *expected_rows]
            )
            observations.append({
                "profile": database.stem,
                "case": label,
                "command": command_record(command),
                "exit_code": result.returncode,
                "stderr": result.stderr,
                "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
                "expected_rows": len(expected),
                "expected_rows_sha256": hashlib.sha256(json.dumps(expected).encode()).hexdigest(),
                "matches": matches,
            })
            if not matches:
                observations[-1]["stdout"] = result.stdout
            if matches:
                compared_rows += len(expected)
                compared_cells += 2 * len(expected)

    after = {relative(path): digest(path) for path in pinned}
    if before != after or compiled_sources(args.revision) != compiled:
        raise RuntimeError("Pinned tools, original bytes or compiled revision changed during replay")
    report = {
        "date": "2026-10-25",
        "scope": "Original inert native getters and independent integer join model; no Access SQL oracle or timing claim",
        "compiled_revision": args.revision,
        "pins_before": before,
        "pins_after": after,
        "compiled_revision_source_sha256": compiled,
        "source_note": "Scoped git-show revision snapshots; mutable live production is not pinned",
        "getters": getter_observations,
        "controls": len(observations),
        "disagreements": sum(not item["matches"] for item in observations),
        "compared_rows": compared_rows,
        "compared_cells": compared_cells,
        "observations": observations,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as output:
        output.write(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in ["controls", "disagreements", "compared_rows", "compared_cells"]}))
    if report["disagreements"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
