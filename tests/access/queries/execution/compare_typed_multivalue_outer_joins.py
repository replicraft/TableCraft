"""Compare outer parent/list results with immutable original getter values."""
import argparse
import json
from pathlib import Path
import subprocess

import compare_bare_multivalue_results as original
from compare_ignored_null_indexes import ROOT, digest, pin


def cases(model):
    identifiers = sorted(model["Ints"])
    matches = [(left, right) for left in identifiers for right in identifiers if left < right]
    left_pairs = matches + [(identifiers[-1], None)]
    right_pairs = matches + [(None, identifiers[0])]

    def scalar(value):
        return "N" if value is None else f"I32:{value}"

    def values(column, identifier):
        return "N" if identifier is None else model[column][identifier]

    def rows(pairs, right_column="Texts"):
        return [
            [scalar(left), scalar(right), values("Ints", left), values(right_column, right)]
            for left, right in pairs
        ]

    headings = ["L", "R", "A", "B"]
    projection = "SELECT a.ID AS L,b.ID AS R,a.Ints AS A,b.Texts AS B "
    for kind, pairs in [("LEFT", left_pairs), ("RIGHT", right_pairs)]:
        yield (
            kind.lower() + "-inequality",
            projection + f"FROM Multivalues a {kind} JOIN Multivalues b ON a.ID<b.ID",
            headings, rows(pairs), None,
        )
    for kind, pairs in [
        ("LEFT", [(value, None) for value in identifiers]),
        ("RIGHT", [(None, value) for value in identifiers]),
    ]:
        yield (
            kind.lower() + "-no-matches",
            projection + f"FROM Multivalues a {kind} JOIN Multivalues b ON False",
            headings, rows(pairs), None,
        )
    yield (
        "left-empty-right",
        projection + "FROM Multivalues a LEFT JOIN "
        "(SELECT ID,Texts FROM Multivalues WHERE ID=0) b ON a.ID=b.ID",
        headings, rows([(value, None) for value in identifiers]), None,
    )
    yield (
        "right-empty-left",
        projection + "FROM (SELECT ID,Ints FROM Multivalues WHERE ID=0) a "
        "RIGHT JOIN Multivalues b ON a.ID=b.ID",
        headings, rows([(None, value) for value in identifiers]), None,
    )
    nested = []
    for left in identifiers:
        selected = [(middle, right) for middle, right in right_pairs if left == middle]
        for middle, right in selected or [(None, None)]:
            nested.append([
                scalar(left), values("Ints", left),
                values("Ints", middle), values("Texts", right),
            ])
    yield (
        "nested-missing-subtree",
        "SELECT a.ID AS ID,a.Ints AS A,b.Ints AS B,c.Texts AS C FROM Multivalues a "
        "LEFT JOIN (Multivalues b RIGHT JOIN Multivalues c ON b.ID<c.ID) ON a.ID=b.ID",
        ["ID", "A", "B", "C"], nested, None,
    )
    shifted = [(left, left - 1 if left - 1 in identifiers else None) for left in identifiers]
    yield (
        "derived-repeated-absence",
        "SELECT q.ID,q.Kept,q.Maybe,q.Maybe AS Again FROM "
        "(SELECT a.ID,a.Ints AS Kept,b.Texts AS Maybe FROM Multivalues a "
        "LEFT JOIN Multivalues b ON a.ID=b.ID+1) q ORDER BY q.ID",
        ["ID", "Kept", "Maybe", "Again"], [
            [scalar(left), values("Ints", left), values("Texts", right), values("Texts", right)]
            for left, right in shifted
        ], None,
    )
    hidden_projection = "SELECT a.ID AS L,b.ID AS R,a.Ints AS A,b.Ints AS B "
    hidden_source = "FROM Multivalues a LEFT JOIN Multivalues b ON a.ID=b.ID+1 "
    positive = [
        pair for pair in shifted
        if pair[1] is not None and "I32:0" in model["Ints"][pair[1]]
    ]
    negative = [
        pair for pair in shifted
        if pair[1] is not None
        and "I32:999" not in model["Ints"][pair[1]]
        and "N" not in model["Ints"][pair[1]]
    ]
    yield (
        "hidden-membership-complete-lists",
        hidden_projection + hidden_source + "WHERE b.Ints.Value=0 ORDER BY a.ID",
        headings, rows(positive, "Ints"), None,
    )
    yield (
        "hidden-not-distinguishes-empty-and-absent",
        hidden_projection + hidden_source + "WHERE NOT (b.Ints.Value=999) ORDER BY a.ID",
        headings, rows(negative, "Ints"), None,
    )
    for name, sql, message in [
        ("mixed-member-refusal", "SELECT a.Ints,b.Texts.Value FROM Multivalues a "
         "LEFT JOIN Multivalues b ON a.ID=b.ID", "not implemented"),
        ("list-on-refusal", "SELECT a.Ints FROM Multivalues a "
         "LEFT JOIN Multivalues b ON a.Ints=b.Ints", "not implemented"),
        ("bare-list-presence-refusal", "SELECT a.Ints,b.Ints FROM Multivalues a "
         "LEFT JOIN Multivalues b ON a.ID=b.ID WHERE b.Ints IS NULL", "not implemented"),
        ("eager-on-error", "SELECT a.Ints,b.Ints FROM Multivalues a "
         "LEFT JOIN Multivalues b ON 1/(a.ID-b.ID)=0 WHERE False", "division by zero"),
    ]:
        yield name, sql, [], [], message


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
    absence_path = "crates/query/tablecraft-query/src/execution/collections/joined/missing.rs"
    if absence_path not in snapshots:
        parser.error("Compiled revision does not contain the outer-list implementation")
    artifacts = [
        args.cli, Path(__file__), ROOT / "docs/testing/compare_bare_multivalue_results.py",
        ROOT / "docs/testing/compare_browsing.py", ROOT / "docs/testing/compare_ignored_null_indexes.py",
        ROOT / "docs/testing/results/bare-multivalue-results-2026-10-25.json",
    ]
    registered = {
        item["path"]: item["sha256"]
        for item in json.loads(artifacts[-1].read_text())["immutable_artifacts_before_after"]
    }
    for profile in original.PROFILES:
        database = args.originals / f"{profile}.accdb"
        if digest(database.read_bytes()) != original.ORIGINAL_SHA256[profile]:
            raise ValueError("Unregistered or changed original database")
        artifacts.append(database)
        for column in original.COLUMNS:
            getter = args.originals / f"{profile}-{column}-getter.stdout"
            relative = getter.resolve().relative_to(ROOT).as_posix()
            if digest(getter.read_bytes()) != registered[relative]:
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
        "scope": "Typed LEFT/RIGHT whole-list transport from original getter models; no native Access SQL execution",
        "controls": len(observations),
        "disagreements": sum(not item["agrees"] for item in observations),
        "rows_compared": row_count, "cells_compared": cell_count,
        "compiled_revision": revision, "selected_compiled_source_objects": snapshots,
        "immutable_artifacts_before_after": before, "raw_log": pin(args.log),
        "observations": observations,
        "limitations": [
            "Selected committed source objects are not a complete compiler-input manifest.",
            "CLI NULL distinguishes absence from bracketed lists but does not prove the typed API variant; separate API tests do.",
            "Existing NaN payload/sign text-display limits and native ordering qualification remain.",
        ],
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"controls": len(observations), "disagreements": report["disagreements"]}))
    raise SystemExit(bool(report["disagreements"]))


if __name__ == "__main__":
    main()
