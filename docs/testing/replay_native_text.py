"""Replay validated native text captures through the CLI, retaining every outcome.

This is an engineering comparison, not a collation implementation. A matching
ordinal probe does not qualify Access locale semantics beyond that observation.
"""
import argparse
from collections import Counter
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

import native_text_oracle as oracle

MAX_OUTPUT_BYTES = 1_048_576
TIMEOUT_SECONDS = 30


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def text_value(units):
    raw = b"".join(int(units[i:i + 4], 16).to_bytes(2, "little")
                   for i in range(0, len(units), 4))
    # A lone surrogate cannot be represented by the CLI's UTF-8 Text carrier.
    return raw.decode("utf-16-le")


def cell_display(cell):
    kind = cell["kind"]
    if kind == "null":
        return "NULL"
    if kind == "boolean":
        return "TRUE" if cell["value"] else "FALSE"
    if kind == "integer":
        return cell["value"]
    if kind != "text":
        raise ValueError("unsupported native carrier")
    escapes = {"\\": "\\\\", '"': '\\"', "\t": "\\t", "\r": "\\r", "\n": "\\n"}
    value = "".join(escapes.get(ch, f"\\u{{{ord(ch):04x}}}" if ord(ch) < 32 or 127 <= ord(ch) <= 159 else ch)
                    for ch in text_value(cell["value"]))
    return '"' + value + '"'


def decode_output(raw):
    if len(raw) > MAX_OUTPUT_BYTES:
        raise ValueError("CLI output exceeds comparison ceiling")
    previous = csv.field_size_limit()
    try:
        csv.field_size_limit(max(previous, MAX_OUTPUT_BYTES))
        rows = list(csv.reader(io.StringIO(raw.decode("utf-8"), newline=""), delimiter="\t", strict=True))
    finally:
        csv.field_size_limit(previous)
    if not rows or not rows[0] or len(rows) - 1 > oracle.MAX_ROWS:
        raise ValueError("missing header or excessive CLI rows")
    if any(len(row) != len(rows[0]) for row in rows[1:]):
        raise ValueError("CLI row width differs from header")
    return rows[0], rows[1:]


def distinct_classes(table, pairs):
    """Use observed equality only when it actually partitions this input corpus.

    Long Text DISTINCT can have operation-specific rules. If native DISTINCT
    does not select exactly one member of each observed equality class, the
    caller must retain an unresolved representative difference, not guess.
    """
    if pairs is None or pairs["status"] != "ok":
        return None
    values = ["NULL" if value is None else cell_display({"kind": "text", "value": value})
              for value in table["values"]]
    count = len(values)
    equal = [[False] * count for _ in range(count)]
    for row in pairs["rows"]:
        left, right = (int(cell["value"]) for cell in row[:2])
        eq = row[2]
        equal[left][right] = eq == {"kind": "boolean", "value": True} or (
            table["values"][left] is None and table["values"][right] is None and eq["kind"] == "null")
    groups = []
    remaining = set(range(count))
    while remaining:
        first = min(remaining)
        group = {i for i in range(count) if equal[first][i]}
        if not group or not group <= remaining:
            return None
        if any({j for j in range(count) if equal[i][j]} != group for i in group):
            return None
        remaining -= group
        groups.append(group)
    classes = {}
    for number, group in enumerate(groups):
        for i in group:
            if values[i] in classes and classes[values[i]] != number:
                return None
            classes[values[i]] = number
    return classes


def compare_rows(case, columns, rows, table, pairs):
    if columns != case["columns"]:
        return "columns_mismatch"
    expected = [[cell_display(cell) for cell in row] for row in case["rows"]]
    if not case["id"].endswith(".distinct"):
        return "match" if rows == expected else "rows_mismatch"
    if Counter(map(tuple, rows)) == Counter(map(tuple, expected)):
        return "match"
    if len(rows) != len(expected) or any(len(row) != 1 for row in rows):
        return "rows_mismatch"
    classes = distinct_classes(table, pairs)
    if classes is None:
        return "distinct_representatives_unresolved"
    def selected(items):
        return Counter(classes.get(row[0], -1) for row in items)
    native = selected(expected)
    if native != Counter({number: 1 for number in classes.values()}):
        return "distinct_representatives_unresolved"
    return "match_equivalent_representatives" if selected(rows) == native else "rows_mismatch"


def replay(directory, cli, collation, like_mode, run=subprocess.run):
    oracle.validate(directory)
    capture = directory / "capture"
    evidence_path = capture / "results.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8-sig"))
    protected = [directory / "request.json", evidence_path, cli]
    protected += [capture / (database["id"] + ".accdb") for database in evidence["databases"]]
    hashes = {path: digest(path) for path in protected}
    report = {"format": "tablecraft-native-text-replay-v1", "cli_sha256": hashes[cli],
              "capture_sha256": hashes[evidence_path], "collation": collation,
              "like_mode": like_mode, "results": []}
    try:
        for database in evidence["databases"]:
            cases = {case["id"]: case for case in database["results"]}
            for probe in oracle.request()["cases"]:
                native = cases[probe["id"]]
                result = {"database": database["id"], "id": probe["id"], "native_status": native["status"]}
                command = [str(cli), "query", "run", probe["sql"], "--database",
                           str(capture / (database["id"] + ".accdb")),
                           "--collation", collation, "--like-mode", like_mode, "--format", "tsv"]
                if "parameter" in probe:
                    command += ["--param", "P=text:" + text_value(probe["parameter"])]
                try:
                    completed = run(command, capture_output=True, timeout=TIMEOUT_SECONDS)
                    result["returncode"] = completed.returncode
                    result["stdout_sha256"] = hashlib.sha256(completed.stdout).hexdigest()
                    result["stderr_sha256"] = hashlib.sha256(completed.stderr).hexdigest()
                    if native["status"] == "error":
                        result.update(status="native_error_unresolved", hresult=native["hresult"], dao_numbers=native["dao_numbers"])
                        if completed.returncode and completed.stdout:
                            result["status"] = "partial_output_on_error"
                    elif completed.returncode:
                        result["status"] = "partial_output_on_error" if completed.stdout else "candidate_error"
                    else:
                        columns, rows = decode_output(completed.stdout)
                        table_name = probe["id"].split(".")[0]
                        table = next(t for t in oracle.request()["tables"] if t["name"] == table_name)
                        result["status"] = compare_rows(native, columns, rows, table, cases.get(table_name + ".pairs"))
                except subprocess.TimeoutExpired:
                    result["status"] = "timeout"
                except (UnicodeError, ValueError, csv.Error) as error:
                    result.update(status="invalid_output", reason=str(error))
                report["results"].append(result)
    finally:
        for path, before in hashes.items():
            oracle.require(digest(path) == before, "replay changed protected input: " + path.name)
    report["counts"] = dict(Counter(result["status"] for result in report["results"]))
    report["all_matched"] = all(result["status"] in ["match", "match_equivalent_representatives"] for result in report["results"])
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--collation", required=True, help="Explicit CLI policy; currently ordinal, not native Access")
    parser.add_argument("--like-mode", choices=["ansi89", "ansi92"], required=True)
    args = parser.parse_args()
    report = replay(args.directory.resolve(), args.cli.resolve(), args.collation, args.like_mode)
    print(json.dumps(report, indent=2))
    raise SystemExit(0 if report["all_matched"] else 1)


if __name__ == "__main__":
    main()
