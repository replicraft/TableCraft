"""Replay native progressive DISTINCT against reviewed ordinary getter records.

No fixture is generated or changed. Only TableCraft executes SQL. Buffered and
progressive commands each compare with independent first-encounter models, not
with each other as an oracle. This is not native Microsoft Access qualification.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

from compare_ignored_null_indexes import JAR_SHA256, ORIGINAL_SHA256, PROFILES, parse_getter
from compare_progressive_derived import compiled_sources

ROOT = Path(__file__).resolve().parents[2]
MAX_COMMANDS = 200
PROVENANCE = "docs/testing/results/ignored-null-index-replay-2026-10-25.json"
PROVENANCE_SHA256 = "18d024ef1ee766dfb7f8410e4a99cb454a5a521bcdb64de2626aca32b37c35b0"
EXTRA_SOURCE_PATHS = (
    "crates/query/tablecraft-query/src/execution/retrieval/finish.rs",
    "crates/query/tablecraft-query/src/execution/retrieval/union.rs",
    "crates/query/tablecraft-query/src/execution/keys.rs",
    "crates/query/tablecraft-query/src/execution/values.rs",
    "crates/query/tablecraft-query/src/output_types.rs",
)
TOOL_PATHS = (
    "docs/testing/compare_progressive_distinct.py",
    "docs/testing/compare_ignored_null_indexes.py",
    "docs/testing/compare_progressive_derived.py",
    "docs/testing/compare_progressive_grouped.py",
    "docs/testing/compare_large_deployment.py",
    "docs/testing/build_large_deployment.py",
    "docs/testing/IgnoreNullIndexFixture.java",
)


def relative(path):
    return Path(path).resolve().relative_to(ROOT).as_posix()


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def invoke(arguments):
    return subprocess.run(
        [str(argument) for argument in arguments],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )


def unique(rows):
    retained = []
    for row in rows:
        if not any(row == previous for previous in retained):
            retained.append(row)
    return retained


def groups(rows):
    retained = []
    for row in rows:
        key = row["key"]
        for previous, members in retained:
            if key == previous:
                members.append(row)
                break
        else:
            retained.append((key, [row]))
    return retained


def scalar(value):
    return "NULL" if value is None else str(value)


def cases(model):
    rows = model["rows"]
    keys = unique([[row["key"]] for row in rows])
    yield {
        "name": "numeric-null-first-encounter",
        "sql": "SELECT DISTINCT KeyValue AS K FROM Ignored",
        "expected": [["K"], *[[scalar(key)] for [key] in keys]],
    }

    tuples = unique([[row["key"], row["id"] % 3] for row in rows])
    yield {
        "name": "two-column-null-tuples",
        "sql": "SELECT DISTINCT KeyValue AS K,CLng(ID Mod 3) AS R FROM Ignored",
        "expected": [["K", "R"], *[[scalar(key), str(remainder)] for key, remainder in tuples]],
    }

    non_null = [row for row in rows if row["key"] is not None]
    first_seven = unique([[row["key"]] for row in non_null])[:7]
    yield {
        "name": "currency-decimal-top-after-dedup",
        "sql": (
            "SELECT DISTINCT TOP 7 CCur(KeyValue) AS C,CDec(KeyValue) AS D,"
            "VarType(CCur(KeyValue)) AS CT,VarType(CDec(KeyValue)) AS DT "
            "FROM Ignored WHERE KeyValue IS NOT NULL"
        ),
        "expected": [
            ["C", "D", "CT", "DT"],
            *[[f"{key}.0000", str(key), "6", "14"] for [key] in first_seven],
        ],
    }

    upstream = unique([[row["key"]] for row in non_null[:30]])
    yield {
        "name": "derived-top-precedes-terminal-distinct",
        "sql": (
            "SELECT DISTINCT K FROM (SELECT TOP 30 CLng(KeyValue) AS K "
            "FROM Ignored WHERE KeyValue IS NOT NULL) Q"
        ),
        "expected": [["K"], *[[str(key)] for [key] in upstream]],
    }

    counts = unique([[len(members)] for _, members in groups(rows)])
    yield {
        "name": "grouped-distinct-count-and-empty-global-union",
        "sql": (
            "SELECT DISTINCT Count(*) AS N FROM Ignored GROUP BY KeyValue "
            "UNION ALL SELECT DISTINCT Count(*) FROM Ignored WHERE False"
        ),
        "expected": [["N"], *[[str(count)] for [count] in counts], ["0"]],
    }

    yield {
        "name": "unsupported-text-profile",
        "sql": "SELECT DISTINCT CStr(KeyValue) FROM Ignored",
        "error": "query feature is not implemented",
    }
    yield {
        "name": "late-eager-error-despite-distinct-top",
        "sql": "SELECT DISTINCT TOP 1 CLng(10\\(ID-700)) AS K FROM Ignored",
        "error": "division by zero",
    }
    yield {
        "name": "later-arm-admission-before-earlier-output",
        "sql": (
            "SELECT ID AS K FROM Ignored WHERE ID=0 UNION ALL "
            "SELECT DISTINCT CDbl(KeyValue) FROM Ignored WHERE False"
        ),
        "error": "query feature is not implemented",
    }


def selected_sources(revision):
    sources = compiled_sources(revision)
    for name in EXTRA_SOURCE_PATHS:
        result = subprocess.run(
            ["git", "show", f"{revision}:{name}"],
            cwd=ROOT,
            capture_output=True,
            check=True,
            timeout=120,
        )
        sources[name] = hashlib.sha256(result.stdout).hexdigest()
    if not any(name.endswith("progressive/distinct.rs") for name in sources):
        raise ValueError("Compiled revision does not contain progressive DISTINCT")
    return {
        name: value for name, value in sources.items()
        if name.startswith("crates/query/tablecraft-query/")
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--originals", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    args = parser.parse_args()
    relative(args.report)
    relative(args.log)
    if args.report.exists() or args.log.exists():
        raise FileExistsError("Refusing to overwrite replay output")
    if len(args.cli_sha256) != 64 or digest(args.cli) != args.cli_sha256:
        raise ValueError("Frozen executable does not match supplied SHA256")
    if digest(args.jar) != JAR_SHA256:
        raise ValueError("Reviewed JAR hash differs")
    provenance_path = ROOT / PROVENANCE
    if digest(provenance_path) != PROVENANCE_SHA256:
        raise ValueError("Reviewed getter provenance differs")
    provenance = json.loads(provenance_path.read_text())
    prior = {Path(pin["path"]).name: pin["sha256"] for pin in provenance["immutable_pins_before"]}
    models = {}
    pinned = [args.cli, args.jar, provenance_path, *(ROOT / name for name in TOOL_PATHS)]
    for profile in PROFILES:
        original = args.originals / f"Ignore-{profile}.accdb"
        getter = args.originals / f"{profile}-getter.tsv"
        if digest(original) != ORIGINAL_SHA256[profile] or digest(getter) != prior[getter.name]:
            raise ValueError(f"Registered original/getter changed: {profile}")
        models[profile] = parse_getter(getter.read_text())
        assert {row["id"] for row in models[profile]["rows"]} == set(range(701))
        pinned.extend([original, getter])
    before = {relative(path): digest(path) for path in pinned}
    sources = selected_sources(args.revision)
    live_query_before = {name: digest(ROOT / name) for name in sources}
    if live_query_before != sources:
        raise ValueError("Selected live query sources differ from the supplied revision")
    expected_commands = len(PROFILES) * 2 * (5 * 4 + 3)
    if expected_commands > MAX_COMMANDS:
        raise ValueError("Command ceiling exceeded before replay")
    args.log.parent.mkdir(parents=True, exist_ok=True)
    records = []
    with args.log.open("x") as log:
        for profile in PROFILES:
            original = args.originals / f"Ignore-{profile}.accdb"
            for declared in [False, True]:
                for case in cases(models[profile]):
                    modes = [("stream", 1)] if "error" in case else [
                        ("buffered", None),
                        ("stream", 1),
                        ("stream", 2),
                        ("stream", 128),
                    ]
                    for mode, batch in modes:
                        command = [
                            relative(args.cli), "--format", "tsv", "query", "run",
                            case["sql"], "--database", relative(original),
                            "--number-format", "invariant",
                        ]
                        if declared:
                            command.append("--declared-arithmetic")
                        if batch is not None:
                            command.extend(["--stream", "--batch-rows", str(batch)])
                        result = invoke(command)
                        actual = list(csv.reader(io.StringIO(result.stdout), delimiter="\t"))
                        error = case.get("error")
                        matched = (
                            result.returncode != 0 and result.stdout == "" and error in result.stderr.lower()
                            if error is not None else
                            result.returncode == 0 and result.stderr == "" and actual == case["expected"]
                        )
                        record = {
                            "profile": profile,
                            "case": case["name"],
                            "declared_arithmetic": declared,
                            "mode": mode,
                            "batch_rows": batch,
                            "command": command,
                            "expected": case.get("expected"),
                            "expected_error_fragment": error,
                            "exit_code": result.returncode,
                            "stdout": result.stdout,
                            "stderr": result.stderr,
                            "matched": matched,
                        }
                        records.append(record)
                        log.write(json.dumps(record) + "\n")
                        log.flush()
    assert len(records) == expected_commands
    after = {relative(path): digest(path) for path in pinned}
    live_query_after = {name: digest(ROOT / name) for name in sources}
    report = {
        "scope": "Original-file progressive fixed-carrier DISTINCT engineering replay; no native Access SQL oracle or timing claims",
        "selected_query_revision": args.revision,
        "selected_query_source_sha256": sources,
        "selected_query_live_sha256_before": live_query_before,
        "selected_query_live_sha256_after": live_query_after,
        "immutable_sha256_before": before,
        "immutable_sha256_after": after,
        "pins_unchanged": before == after and live_query_before == live_query_after == sources,
        "getter_scope": "Reused prior reviewed public ordinary-getter transcripts; no fixture authoring or new Java evaluation",
        "commands": records,
        "cli_commands": len(records),
        "success_commands": sum(record["expected_error_fragment"] is None for record in records),
        "failure_controls": sum(record["expected_error_fragment"] is not None for record in records),
        "disagreements": sum(not record["matched"] for record in records),
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(json.dumps({name: report[name] for name in [
        "cli_commands", "success_commands", "failure_controls", "disagreements", "pins_unchanged"
    ]}, indent=2))
    if report["disagreements"] or not report["pins_unchanged"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
