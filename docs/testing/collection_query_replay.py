"""Replay explicit collection query cases against registered original getters."""
import argparse
import json
from pathlib import Path
import subprocess

import compare_bare_multivalue_results as original
from compare_ignored_null_indexes import ROOT, digest, pin


def replay(case_factory, scope, case_file, required_sources, extra_options):
    parser = argparse.ArgumentParser(description=scope)
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
    if not set(required_sources).issubset(snapshots):
        parser.error("Compiled revision lacks a required collection source module")
    artifacts = [
        args.cli, Path(case_file), Path(__file__),
        ROOT / "docs/testing/compare_bare_multivalue_results.py",
        ROOT / "docs/testing/compare_browsing.py",
        ROOT / "docs/testing/compare_ignored_null_indexes.py",
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
            cases = list(case_factory(model))
            if len({case[0] for case in cases}) != len(cases):
                raise ValueError("Case names must be unique within a profile")
            for declared in (False, True):
                for name, sql, headings, expected, refusal in cases:
                    if any(len(row) != len(headings) for row in expected):
                        raise ValueError("Expected row width does not match headings")
                    command = [
                        str(args.cli), "--format", "tsv", "query", "run", sql,
                        "--database", str(args.originals / f"{profile}.accdb"),
                        "--multivalues", "--native-bytes", "67108864", *extra_options,
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
        "scope": scope, "controls": len(observations),
        "disagreements": sum(not item["agrees"] for item in observations),
        "rows_compared": row_count, "cells_compared": cell_count,
        "compiled_revision": revision, "selected_compiled_source_objects": snapshots,
        "immutable_artifacts_before_after": before, "raw_log": pin(args.log),
        "observations": observations,
        "limitations": [
            "Selected committed source objects are not a complete compiler-input manifest.",
            "Getter models establish stored data, not native Access SQL or locale behavior.",
            "Existing NaN payload/sign text-display limits remain; typed API tests cover exact stored bits.",
        ],
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"controls": len(observations), "disagreements": report["disagreements"]}))
    raise SystemExit(bool(report["disagreements"]))
