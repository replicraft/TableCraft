"""Replay typed INNER/CROSS list transport against unchanged native getter models."""
import argparse
import json
from pathlib import Path
import subprocess

import compare_bare_multivalue_results as original
from compare_ignored_null_indexes import ROOT, digest, pin


def cases(model):
    identifiers = sorted(model["Ints"])
    equal = [(left, right) for left in identifiers for right in identifiers if left == right]
    ordered = [(left, right) for left in identifiers for right in identifiers if left <= right]
    product = [(left, right) for left in identifiers for right in identifiers]

    def output(pairs, left_column="Ints", right_column="Texts"):
        return [
            [f"I32:{left}", f"I32:{right}", model[left_column][left], model[right_column][right]]
            for left, right in pairs
        ]

    projection = "SELECT a.ID AS L,b.ID AS R,a.Ints AS A,b.Texts AS B "
    table = "FROM Multivalues a INNER JOIN Multivalues b "
    yield "inner-equal", projection + table + "ON a.ID=b.ID ORDER BY a.ID,b.ID", ["L", "R", "A", "B"], output(equal), None
    yield "inner-inequality", projection + table + "ON a.ID<=b.ID ORDER BY a.ID,b.ID", ["L", "R", "A", "B"], output(ordered), None
    limited = [(left, right) for left, right in product if left <= 2 and right <= 2]
    yield "cross-filter", projection + "FROM Multivalues a,Multivalues b WHERE a.ID<=2 AND b.ID<=2 ORDER BY a.ID,b.ID", ["L", "R", "A", "B"], output(limited), None
    yield "swapped-lists", "SELECT a.ID AS L,b.ID AS R,b.Texts AS A,a.Ints AS B " + table + "ON a.ID=b.ID ORDER BY a.ID", ["L", "R", "A", "B"], [
        [f"I32:{left}", f"I32:{right}", model["Texts"][right], model["Ints"][left]] for left, right in equal
    ], None
    yield "nested-right", (
        "SELECT a.ID AS ID,a.Bytes AS A,b.Ints AS B,c.Texts AS C FROM Multivalues a "
        "INNER JOIN (Multivalues b INNER JOIN Multivalues c ON b.ID=c.ID) "
        "ON a.ID=b.ID ORDER BY a.ID"
    ), ["ID", "A", "B", "C"], [
        [f"I32:{value}", model["Bytes"][value], model["Ints"][value], model["Texts"][value]]
        for value in identifiers
    ], None
    yield "scalar-derived-leaf", (
        "SELECT a.ID AS ID,a.Singles AS A FROM Multivalues a "
        "INNER JOIN (SELECT 1 AS K) b ON a.ID=b.K"
    ), ["ID", "A"], [["I32:1", model["Singles"][1]]], None
    matching = [
        (left, right) for left, right in ordered
        if "I32:0" in model["Ints"][left] and "I32:1" in model["Ints"][right]
    ]
    yield "hidden-both-complete-lists", (
        "SELECT a.ID AS L,b.ID AS R,a.Ints AS A,b.Ints AS B " + table
        + "ON a.ID<=b.ID WHERE a.Ints.Value=0 AND b.Ints.Value=1 ORDER BY a.ID,b.ID"
    ), ["L", "R", "A", "B"], output(matching, "Ints", "Ints"), None
    yield "typed-derived-leaf", (
        "SELECT q.ID AS L,b.ID AS R,q.ValuesList AS A,b.Texts AS B "
        "FROM (SELECT ID,Ints AS ValuesList FROM Multivalues) q "
        "INNER JOIN Multivalues b ON q.ID=b.ID ORDER BY q.ID"
    ), ["L", "R", "A", "B"], output(equal), None
    yield "outer-refusal", projection + "FROM Multivalues a LEFT JOIN Multivalues b ON a.ID=b.ID", [], [], "not implemented"
    yield "mixed-member-refusal", (
        "SELECT a.Ints,b.Texts.Value FROM Multivalues a INNER JOIN Multivalues b ON a.ID=b.ID"
    ), [], [], "not implemented"
    yield "list-comparison-refusal", (
        "SELECT a.Ints FROM Multivalues a INNER JOIN Multivalues b ON a.Ints=b.Ints"
    ), [], [], "not implemented"
    yield "eager-on-before-false-where", (
        "SELECT a.Ints FROM Multivalues a INNER JOIN Multivalues b "
        "ON 1/(a.ID-b.ID)=0 WHERE 1=0"
    ), [], [], "division by zero"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "originals", "report", "log"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--compiled-revision", required=True)
    args = parser.parse_args()
    if args.report.exists() or args.log.exists():
        parser.error("Report and log paths must be fresh")
    if digest(args.cli.read_bytes()) != args.cli_sha256:
        parser.error("Frozen CLI checksum mismatch")
    revision = subprocess.check_output(
        ["git", "rev-parse", args.compiled_revision], cwd=ROOT, text=True, timeout=30,
    ).strip()
    snapshots = original.source_objects(revision)
    joined_sources = {
        "crates/query/tablecraft-query/src/execution/collections/joined/mod.rs",
        "crates/query/tablecraft-query/src/execution/collections/joined/relation.rs",
        "crates/query/tablecraft-query/src/execution/collections/joined/table.rs",
    }
    if not joined_sources.issubset(snapshots):
        parser.error("Compiled revision does not include the typed join implementation")
    artifacts = [
        args.cli, Path(__file__), ROOT / "docs/testing/compare_bare_multivalue_results.py",
        ROOT / "docs/testing/compare_browsing.py", ROOT / "docs/testing/compare_ignored_null_indexes.py",
        ROOT / "docs/testing/results/bare-multivalue-results-2026-10-25.json",
    ]
    provenance = json.loads(artifacts[-1].read_text())
    registered = {item["path"]: item["sha256"] for item in provenance["immutable_artifacts_before_after"]}
    for profile in original.PROFILES:
        database = args.originals / f"{profile}.accdb"
        if digest(database.read_bytes()) != original.ORIGINAL_SHA256[profile]:
            raise ValueError("Unregistered or changed original database")
        artifacts.append(database)
        for column in original.COLUMNS:
            getter = args.originals / f"{profile}-{column}-getter.stdout"
            if digest(getter.read_bytes()) != registered[getter.resolve().relative_to(ROOT).as_posix()]:
                raise ValueError("Getter differs from registered original transcript")
            artifacts.append(getter)
    before = [pin(path) for path in artifacts]
    observations = []
    row_count = cell_count = 0
    with args.log.open("x", encoding="utf-8") as log:
        for profile in original.PROFILES:
            model = {
                column: original.getter_records(args.originals / f"{profile}-{column}-getter.stdout")
                for column in original.COLUMNS
            }
            for declared in (False, True):
                for name, sql, headings, expected, refusal in cases(model):
                    command = [
                        str(args.cli), "--format", "tsv", "query", "run", sql,
                        "--database", str(args.originals / f"{profile}.accdb"),
                        "--multivalues", "--native-bytes", "67108864",
                    ]
                    if declared:
                        command.append("--declared-arithmetic")
                    result = subprocess.run(command, capture_output=True, text=True, timeout=60)
                    disagreement = None
                    try:
                        if refusal:
                            assert result.returncode == 2 and not result.stdout
                            assert refusal in result.stderr
                        else:
                            assert result.returncode == 0 and not result.stderr
                            original.compare_table(result.stdout, headings, expected)
                            row_count += len(expected)
                            cell_count += sum(map(len, expected))
                    except AssertionError as error:
                        disagreement = str(error) or "Unexpected exit, result or error class"
                    observation = {
                        "profile": profile, "declared": declared, "case": name,
                        "command": command, "exit_code": result.returncode,
                        "stdout": result.stdout, "stderr": result.stderr,
                        "agrees": disagreement is None, "disagreement": disagreement,
                    }
                    log.write(json.dumps(observation) + "\n")
                    observations.append(observation)
    if [pin(path) for path in artifacts] != before:
        raise ValueError("Original, getter, executable or reference changed during replay")
    report = {
        "scope": "Typed INNER/CROSS complete-list transport using exact original getter values; no native Access SQL execution",
        "controls": len(observations),
        "disagreements": sum(not item["agrees"] for item in observations),
        "rows_compared": row_count, "cells_compared": cell_count,
        "compiled_revision": revision, "selected_compiled_source_objects": snapshots,
        "immutable_artifacts_before_after": before, "raw_log": pin(args.log),
        "observations": observations,
        "limitations": [
            "Selected committed source objects are not a complete compiler input manifest.",
            "Existing typed-list display and NaN payload/sign display limits remain.",
            "Outer joins, list comparisons and visible member/list combinations remain refused.",
        ],
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"controls": len(observations), "disagreements": report["disagreements"]}))
    raise SystemExit(bool(report["disagreements"]))


if __name__ == "__main__":
    main()
