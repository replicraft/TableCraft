"""Prepare and validate original native ACE aggregate value/type observations.

Validation proves artifact completeness, not TableCraft/native equivalence.
Only the fixed original recipe is executable by the Windows collector.
"""
import argparse
from collections import Counter
import csv
from decimal import Decimal
from io import StringIO
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess

FORMAT = "tablecraft-native-aggregates-v1"
MAX_ROWS = 1
COLUMNS = ["N", "Subtype"]
HEADER_PREFIX_BYTES = 128
DATABASE_MAGIC_OFFSET = 4
DATABASE_MAGIC = b"Standard ACE DB"
DAO_BYTE, DAO_INTEGER, DAO_LONG, DAO_CURRENCY = 2, 3, 4, 5
DAO_SINGLE, DAO_DOUBLE, DAO_BIGINT, DAO_NUMERIC, DAO_DECIMAL = 6, 7, 16, 19, 20
DAO_INTEGER_RANGES = {
    DAO_BYTE: (0, 255), DAO_INTEGER: (-(1 << 15), (1 << 15) - 1),
    DAO_LONG: (-(1 << 31), (1 << 31) - 1), DAO_BIGINT: (-(1 << 63), (1 << 63) - 1),
}
DAO_KINDS = {
    DAO_CURRENCY: "decimal", DAO_SINGLE: "single", DAO_DOUBLE: "double",
    DAO_NUMERIC: "decimal", DAO_DECIMAL: "decimal",
}
DECIMAL_WORD_BITS = 32
DECIMAL_WORD_HEX = DECIMAL_WORD_BITS // 4
DECIMAL_WORD_COUNT = 4
DECIMAL_FLAGS_OFFSET = (DECIMAL_WORD_COUNT - 1) * DECIMAL_WORD_HEX
DECIMAL_ALLOWED_FLAGS = 0x80FF0000
DECIMAL_SIGN = 0x80000000
DECIMAL_SCALE_SHIFT = 16
DECIMAL_SCALE_MASK = 255
DECIMAL_MAX_SCALE = 28
HEX_DIGEST = re.compile(r"[0-9a-f]{64}\Z")
INTEGER = re.compile(r"-?(?:0|[1-9][0-9]*)\Z")


def request():
    cases = []
    for function in ["Count", "Sum", "Avg", "Min", "Max", "First", "Last"]:
        for cast in ["CByte", "CInt", "CLng", "CSng", "CDbl", "CCur", "CDec"]:
            expression = f"{function}({cast}(V))"
            for phase, condition in [("full", "V IS NOT NULL"), ("empty", "False")]:
                cases.append({
                    "id": f"{function}.{cast}.{phase}",
                    "sql": f"SELECT {expression} AS N, VarType({expression}) AS Subtype FROM ProbeData WHERE {condition}",
                })
    for expression in ["Count(*)", "Count(V)"]:
        cases.append({"id": expression, "sql": f"SELECT {expression} AS N, VarType({expression}) AS Subtype FROM ProbeData"})
    return {"format": FORMAT, "max_rows": MAX_ROWS,
            "locale": ";LANGID=0x0409;CP=1252;COUNTRY=0",
            "values": [1, 2, 3, None], "cases": cases}


def request_bytes():
    return (json.dumps(request(), ensure_ascii=True, indent=2) + "\n").encode("ascii")


def prepare(directory):
    directory.mkdir(parents=True, exist_ok=False)
    (directory / "request.json").write_bytes(request_bytes())


def require(condition, message):
    if not condition:
        raise ValueError(message)


def valid_cell(cell):
    if not isinstance(cell, dict):
        return False
    if cell == {"kind": "null"}:
        return True
    if set(cell) != {"kind", "value"} or not isinstance(cell["value"], str):
        return False
    kind, value = cell["kind"], cell["value"]
    if kind == "integer":
        return bool(INTEGER.fullmatch(value)) and -(1 << 63) <= int(value) < (1 << 63)
    if kind in ["single", "double"]:
        width, code = (8, ">f") if kind == "single" else (16, ">d")
        if re.fullmatch(r"[0-9a-f]{" + str(width) + r"}", value) is None:
            return False
        number = struct.unpack(code, bytes.fromhex(value))[0]
        return float("-inf") < number < float("inf")
    if kind == "decimal":
        if re.fullmatch(r"[0-9a-f]{32}", value) is None:
            return False
        flags = int(value[DECIMAL_FLAGS_OFFSET:], 16)
        return flags & ~DECIMAL_ALLOWED_FLAGS == 0 and ((flags >> DECIMAL_SCALE_SHIFT) & DECIMAL_SCALE_MASK) <= DECIMAL_MAX_SCALE
    return False


def field_accepts(field, cell):
    if cell["kind"] == "null":
        return True
    native = field["dao_type"]
    if native in DAO_INTEGER_RANGES:
        if cell["kind"] != "integer":
            return False
        low, high = DAO_INTEGER_RANGES[native]
        return low <= int(cell["value"]) <= high
    return cell["kind"] == DAO_KINDS.get(native)


def validate(directory):
    require((directory / "request.json").read_bytes() == request_bytes(), "request differs from original recipe")
    capture = directory / "capture"
    report = json.loads((capture / "results.json").read_text(encoding="utf-8-sig"))
    require(report.get("format") == FORMAT, "unknown report format")
    require(report.get("request_sha256") == hashlib.sha256(request_bytes()).hexdigest(), "request hash mismatch")
    runner = Path(__file__).with_name("Capture-NativeAggregates.ps1")
    require(report.get("runner_sha256") == hashlib.sha256(runner.read_bytes()).hexdigest(), "runner hash mismatch")
    environment = report.get("environment", {})
    for key in ["engine_version", "engine_dll_version", "windows_version", "powershell_version"]:
        require(isinstance(environment.get(key), str) and environment[key], f"missing environment {key}")
    require(environment.get("process_bits") in [32, 64], "invalid process bitness")
    database = report.get("database", {})
    raw = (capture / "aggregates.accdb").read_bytes()
    require(len(raw) >= HEADER_PREFIX_BYTES and raw[DATABASE_MAGIC_OFFSET:DATABASE_MAGIC_OFFSET + len(DATABASE_MAGIC)] == DATABASE_MAGIC, "expected ACCDB header")
    require(database.get("bytes") == len(raw), "database length mismatch")
    digest = database.get("sha256", "")
    require(isinstance(digest, str) and HEX_DIGEST.fullmatch(digest), "invalid database hash")
    require(hashlib.sha256(raw).hexdigest() == digest, "database hash mismatch")
    require(database.get("header_hex") == raw[:HEADER_PREFIX_BYTES].hex(), "database header mismatch")
    cases = report.get("results", [])
    require([case.get("id") for case in cases] == [case["id"] for case in request()["cases"]], "missing/reordered/duplicate cases")
    counts = Counter()
    for case in cases:
        status = case.get("status")
        if status == "error":
            require("rows" not in case and "fields" not in case, "partial result retained after error")
            require(type(case.get("hresult")) is int and case["hresult"] < 0, "missing native HRESULT")
            errors = case.get("dao_numbers")
            require(isinstance(errors, list) and errors and all(type(n) is int for n in errors), "missing DAO error numbers")
            counts["errors"] += 1
            continue
        require(status == "ok", "unknown outcome")
        fields = case.get("fields", [])
        require([field.get("name") for field in fields] == COLUMNS, "invalid result columns")
        require(all(type(field.get("dao_type")) is int and field["dao_type"] > 0
                    and type(field.get("size")) is int and field["size"] >= 0 for field in fields), "missing field type/size")
        rows = case.get("rows")
        require(isinstance(rows, list) and len(rows) == MAX_ROWS, "incomplete aggregate rows")
        require(all(isinstance(row, list) and len(row) == len(COLUMNS)
                    and all(valid_cell(cell) for cell in row) for row in rows), "invalid/lossy result cell")
        require(all(field_accepts(field, cell) for field, cell in zip(fields, rows[0], strict=True)), "field/value carrier mismatch")
        require(rows[0][1]["kind"] == "integer" and 0 <= int(rows[0][1]["value"]) <= 65535, "invalid VarType result")
        counts["captured"] += 1
    return counts


def value_matches(cell, text):
    """Compare values separately from native VarType and DAO field metadata."""
    kind = cell["kind"]
    if kind == "null":
        return text == "NULL"
    try:
        if kind == "integer":
            return Decimal(text) == int(cell["value"])
        if kind in ["single", "double"]:
            code = ">f" if kind == "single" else ">d"
            return struct.pack(code, float(text)).hex() == cell["value"]
        if kind == "decimal":
            words = [int(cell["value"][offset:offset + DECIMAL_WORD_HEX], 16)
                     for offset in range(0, DECIMAL_WORD_COUNT * DECIMAL_WORD_HEX, DECIMAL_WORD_HEX)]
            coefficient = words[0] | words[1] << DECIMAL_WORD_BITS | words[2] << (2 * DECIMAL_WORD_BITS)
            if words[3] & DECIMAL_SIGN:
                coefficient = -coefficient
            digits = tuple(int(digit) for digit in str(abs(coefficient)))
            exact = Decimal((int(coefficient < 0), digits, -((words[3] >> DECIMAL_SCALE_SHIFT) & DECIMAL_SCALE_MASK)))
            return Decimal(text) == exact
    except (ValueError, ArithmeticError, OverflowError):
        return False
    return False


def compare(directory, cli):
    validate(directory)
    capture = directory / "capture"
    database = capture / "aggregates.accdb"
    before = hashlib.sha256(database.read_bytes()).hexdigest()
    native = json.loads((capture / "results.json").read_text(encoding="utf-8-sig"))
    counts = Counter()
    outcomes = []
    for probe, observed in zip(request()["cases"], native["results"], strict=True):
        if observed["status"] != "ok":
            counts["native_errors"] += 1
            outcomes.append({"id": probe["id"], "status": "native_error"})
            continue
        try:
            result = subprocess.run(
                [str(cli.resolve()), "query", "run", probe["sql"], "--database", str(database), "--format", "tsv"],
                capture_output=True, text=True, encoding="utf-8", timeout=10, check=False,
            )
        except subprocess.TimeoutExpired:
            counts["tablecraft_errors"] += 1
            outcomes.append({"id": probe["id"], "status": "tablecraft_timeout"})
            continue
        if result.returncode != 0:
            counts["tablecraft_errors"] += 1
            outcomes.append({"id": probe["id"], "status": "tablecraft_error"})
            continue
        try:
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        except csv.Error:
            counts["protocol_errors"] += 1
            outcomes.append({"id": probe["id"], "status": "tablecraft_protocol_error"})
            continue
        if len(rows) != 2 or rows[0] != COLUMNS or len(rows[1]) != len(COLUMNS):
            counts["protocol_errors"] += 1
            outcomes.append({"id": probe["id"], "status": "tablecraft_protocol_error"})
            continue
        same_value = value_matches(observed["rows"][0][0], rows[1][0])
        same_carrier = value_matches(observed["rows"][0][1], rows[1][1])
        counts["compared"] += 1
        counts["value_matches"] += same_value
        counts["carrier_matches"] += same_carrier
        counts["both_match"] += same_value and same_carrier
        outcomes.append({"id": probe["id"], "status": "compared",
                         "same_value": same_value, "same_runtime_subtype": same_carrier})
    require(hashlib.sha256(database.read_bytes()).hexdigest() == before, "TableCraft read changed original capture")
    return {"counts": dict(counts), "outcomes": outcomes}


def exercise(directory, cli):
    """Run the actual recipe over inline inputs, without inventing native types."""
    require((directory / "request.json").read_bytes() == request_bytes(), "request differs from original recipe")
    arms = ["SELECT " + ("NULL" if value is None else f"CLng({value})") + " AS V"
            for value in request()["values"]]
    source = "(" + " UNION ALL ".join(arms) + ") AS ProbeData"
    controls = 0
    for case in request()["cases"]:
        sql = case["sql"].replace("FROM ProbeData", "FROM " + source)
        result = subprocess.run(
            [str(cli.resolve()), "query", "run", sql, "--format", "tsv"],
            capture_output=True, text=True, encoding="utf-8", timeout=10, check=False,
        )
        require(result.returncode == 0, f"TableCraft probe failed: {case['id']}")
        rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        require(len(rows) == 2 and rows[0] == COLUMNS and len(rows[1]) == 2, f"invalid probe output: {case['id']}")
        if case["id"] == "Count(*)":
            expected = 4
        elif case["id"] == "Count(V)":
            expected = 3
        else:
            function, _, phase = case["id"].split(".")
            expected = {"Count": 3, "Sum": 6, "Avg": 2, "Min": 1, "Max": 3, "First": 1, "Last": 3}[function]
            if phase == "empty":
                expected = 0 if function == "Count" else None
        matches = rows[1][0] == "NULL" if expected is None else Decimal(rows[1][0]) == expected
        require(matches, f"aggregate value mismatch: {case['id']}")
        require(INTEGER.fullmatch(rows[1][1]) is not None, f"missing runtime subtype: {case['id']}")
        controls += 1
    return controls


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["prepare", "exercise", "validate", "compare"])
    parser.add_argument("directory", type=Path)
    parser.add_argument("--cli", type=Path)
    args = parser.parse_args()
    if args.command in ["exercise", "compare"] and args.cli is None:
        parser.error("exercise/compare require --cli")
    if args.command == "prepare":
        prepare(args.directory)
        print("Prepared 100 original aggregate probes; native execution remains unrun.")
    elif args.command == "exercise":
        count = exercise(args.directory, args.cli)
        print(f"Passed {count} TableCraft-side aggregate probes on inline inputs; native comparison remains unrun.")
    elif args.command == "compare":
        result = compare(args.directory, args.cli)
        print(json.dumps(result, indent=2))
        return 0 if result["counts"].get("both_match") == len(request()["cases"]) else 1
    else:
        counts = validate(args.directory)
        print(f"Evidence complete: {counts['captured']} results and {counts['errors']} native errors; no compatibility claim.")


if __name__ == "__main__":
    raise SystemExit(main())
