"""Replay complete indexed snapshots over registered 5,000-page originals."""
import argparse
import csv
import io
import json
from pathlib import Path
import subprocess

from compare_ignored_null_indexes import ROOT, digest, pin
from compare_large_index_selections import ORIGINAL_SHA256, ORACLE_SHA256, SOURCE_PATHS, getter_model

ROW_BYTES = 240
BINARY_FIELDS = 12


def field_bytes(identifier, column):
    return bytes((identifier + column + position) % 256 for position in range(ROW_BYTES))


def cases():
    # kind, columns, native row allowance, output byte allowance, SQL, refusal
    yield "all-ids", [], 100000, 16777216, "SELECT ID FROM Pages", None
    yield "descending-ids", [], 100000, 16777216, "SELECT ID FROM Pages ORDER BY ID DESC", None
    yield "count-complete", [], 100000, 16777216, "SELECT Count(*) AS N FROM Pages", None
    yield "hidden-key-projection", [0], 100000, 16777216, "SELECT B0 FROM Pages", None
    for name, columns in [("payload-first-half", list(range(6))), ("payload-second-half", list(range(6, 12)))]:
        sql = "SELECT ID," + ",".join(f"B{column}" for column in columns) + " FROM Pages"
        yield name, columns, 100000, 16777216, sql, None
    yield "complete-cap-refusal", [], 4999, 16777216, "SELECT ID FROM Pages", "limit"
    yield "partial-with-same-cap", [], 4999, 16777216, "SELECT ID FROM Pages WHERE ID=4999", None
    yield "buffered-output-refusal", [], 100000, 1, "SELECT ID FROM Pages", "evaluation output limit exceeded"


def compare(result, kind, columns, rows):
    identifiers = [row["id"] for row in rows]
    table = list(csv.reader(io.StringIO(result), delimiter="\t"))
    if kind == "count-complete":
        assert table == [["N"], [str(len(rows))]]
        return 1
    if not columns:
        if kind == "descending-ids":
            identifiers.reverse()
        elif kind == "partial-with-same-cap":
            identifiers = identifiers[-1:]
        assert table == [["ID"], *[[str(identifier)] for identifier in identifiers]]
        return len(identifiers)
    hidden = kind == "hidden-key-projection"
    headings = ([] if hidden else ["ID"]) + [f"B{column}" for column in columns]
    assert table[0] == headings and len(table) == len(rows) + 1
    for identifier, actual in zip(identifiers, table[1:]):
        assert len(actual) == len(headings)
        values = actual
        if not hidden:
            assert actual[0] == str(identifier)
            values = actual[1:]
        for column, value in zip(columns, values):
            assert value.startswith("0x")
            assert bytes.fromhex(value[2:]) == field_bytes(identifier, column)
    return len(rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "inputs", "report", "log"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--compiled-revision", required=True)
    args = parser.parse_args()
    if args.report.exists() or args.log.exists():
        parser.error("Report and log must be fresh")
    if digest(args.cli.read_bytes()) != args.cli_sha256:
        parser.error("Frozen CLI checksum mismatch")
    revision = subprocess.check_output(
        ["git", "rev-parse", args.compiled_revision], cwd=ROOT, text=True, timeout=30,
    ).strip()
    sources = {}
    for path in SOURCE_PATHS:
        data = subprocess.check_output(["git", "show", f"{revision}:{path}"], cwd=ROOT, timeout=30)
        sources[path] = {"bytes": len(data), "sha256": digest(data)}
    artifacts = [
        args.cli, Path(__file__), ROOT / "docs/testing/compare_large_index_selections.py",
        ROOT / "docs/testing/compare_ignored_null_indexes.py", ROOT / "docs/testing/LargeUsageMapProbe.java",
    ]
    for profile in ORIGINAL_SHA256:
        database = args.inputs / f"{profile}.accdb"
        oracle = args.inputs / f"{profile}-oracle.stdout"
        if digest(database.read_bytes()) != ORIGINAL_SHA256[profile]:
            raise ValueError("Original database differs from the registered fixture")
        if digest(oracle.read_bytes()) != ORACLE_SHA256[profile]:
            raise ValueError("Getter differs from the registered transcript")
        artifacts.extend([database, oracle])
    before = [pin(path) for path in artifacts]
    observations = []
    compared_rows = 0
    payload_fields = 0
    with args.log.open("x", encoding="utf-8") as log:
        for profile in ORIGINAL_SHA256:
            rows = getter_model(args.inputs / f"{profile}-oracle.stdout")
            # The producer formula is checked against the independent getter
            # digest before it is used for exact per-field output comparisons.
            for row in rows:
                payload = b"".join(field_bytes(row["id"], column) for column in range(BINARY_FIELDS))
                assert digest(payload) == row["payload_sha256"]
            for kind, columns, native_rows, output_bytes, sql, refusal in cases():
                command = [
                    str(args.cli), "--format", "tsv", "query", "run", sql,
                    "--database", str(args.inputs / f"{profile}.accdb"),
                    "--index", "Pages", "PrimaryKey", "--native-rows", str(native_rows),
                    "--native-bytes", "268435456", "--native-steps", "100000000",
                    "--execution-bytes", "268435456", "--execution-steps", "100000000",
                    "--output-bytes", str(output_bytes),
                ]
                result = subprocess.run(command, capture_output=True, text=True, timeout=60)
                disagreement = None
                returned = 0
                try:
                    if refusal:
                        assert result.returncode == 2 and not result.stdout
                        assert refusal in result.stderr
                    else:
                        assert result.returncode == 0 and not result.stderr
                        returned = compare(result.stdout, kind, columns, rows)
                        compared_rows += returned
                        payload_fields += returned * len(columns)
                except (AssertionError, ValueError, IndexError) as error:
                    disagreement = str(error) or "Unexpected exit, count, payload or order"
                observation = {
                    "profile": profile, "case": kind, "command": command,
                    "exit_code": result.returncode, "stderr": result.stderr,
                    "stdout_bytes": len(result.stdout.encode()), "stdout_sha256": digest(result.stdout.encode()),
                    "rows_compared": returned, "agrees": disagreement is None,
                    "disagreement": disagreement,
                }
                # Full large payload output remains in the private raw transcript;
                # the public record retains its digest and independently checked counts.
                log.write(json.dumps({**observation, "stdout": result.stdout}) + "\n")
                observations.append(observation)
    if [pin(path) for path in artifacts] != before:
        raise ValueError("Executable, original, getter or reference changed during replay")
    report = {
        "scope": "Explicit complete indexed snapshots over original 5,000-live-row/page ACE inputs; legacy/check/write gates remain separate",
        "controls": len(observations), "disagreements": sum(not row["agrees"] for row in observations),
        "rows_compared": compared_rows, "binary_fields_compared": payload_fields,
        "compiled_revision": revision, "selected_compiled_source_objects": sources,
        "immutable_artifacts_before_after": before, "raw_log": pin(args.log), "observations": observations,
        "limitations": [
            "Independent public-writer/getter evidence is not native Microsoft Access execution or write qualification.",
            "Whole key/reference lists, snapshots and CLI output remain buffered under explicit cumulative limits.",
            "Legacy complete/check/write and physical index-owned map ceilings are not expanded.",
            "Selected source objects are not a complete compiler-input manifest.",
        ],
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"controls": len(observations), "disagreements": report["disagreements"]}))
    raise SystemExit(bool(report["disagreements"]))


if __name__ == "__main__":
    main()
