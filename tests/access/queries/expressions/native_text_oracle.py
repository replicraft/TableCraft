"""Prepare original native ACE text probes and validate captured evidence.

The validator checks completeness, provenance and lossless stored values. It
does not treat a successful capture as TableCraft/native SQL compatibility.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re

FORMAT = "tablecraft-native-text-v1"
HEX = re.compile(r"(?:[0-9a-f]{4})*\Z")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
MAX_ROWS = 4096


def utf16(text):
    raw = text.encode("utf-16-le", errors="surrogatepass")
    return "".join(f"{int.from_bytes(raw[i:i + 2], 'little'):04x}" for i in range(0, len(raw), 2))


def request():
    # These are original inputs, not weights or expected native comparison rules.
    short = [None, "", "a", "A", "a ", "a  ", " a", "b", "é", "e\u0301",
             "e", "E", "ß", "ss", "æ", "ae", "a-b", "ab", "a'b", "a\u00adb",
             "a\u200bb", "a\x00b", "a\tb", "a\nb", "I", "i", "İ", "ı",
             "カ", "か", "ｶ", "ガ", "カ\u3099", "😀", "😁", "x" * 254 + "a",
             "x" * 254 + "b"]
    long = short + ["x" * 255 + "a", "x" * 255 + "b", "x" * 255 + "A",
                    "x" * 255, "x" * 255 + " ", "x" * 254 + "😀a",
                    "x" * 254 + "😀b", "x" * 1024 + "a", "x" * 1024 + "b"]
    tables = []
    cases = []
    for name, kind, values in [("ShortValues", "short", short), ("LongValues", "long", long)]:
        tables.append({"name": name, "kind": kind, "values": [None if x is None else utf16(x) for x in values]})
        def add(suffix, sql):
            cases.append({"id": name + "." + suffix, "sql": sql})
        add("stored", f"SELECT ID, V FROM {name} ORDER BY ID")
        add("pairs", f"SELECT A.ID AS LID, B.ID AS RID, A.V = B.V AS Eq, A.V < B.V AS Lt, A.V > B.V AS Gt FROM {name} AS A, {name} AS B ORDER BY A.ID, B.ID")
        for direction in ["ASC", "DESC"]:
            add("sort_" + direction.lower(), f"SELECT ID FROM {name} ORDER BY V {direction}, ID")
        add("distinct", f"SELECT DISTINCT V FROM {name}")
        add("group", f"SELECT Min(ID) AS FirstID, Count(*) AS N FROM {name} GROUP BY V ORDER BY Min(ID)")
        add("join", f"SELECT A.ID AS LID, B.ID AS RID FROM {name} AS A INNER JOIN {name} AS B ON A.V = B.V ORDER BY A.ID, B.ID")
        for i, pattern in enumerate(["*", "?", "??", "a*", "[a-c]*", "[!a-c]*", "é*", "x*", "*😀?", "%", "_"]):
            literal = pattern.replace("'", "''")
            add(f"like_{i}", f"SELECT ID, V LIKE '{literal}' AS Matched FROM {name} ORDER BY ID")
        add("parameter_text", f"PARAMETERS P TEXT(255); SELECT ID, V = P AS Eq FROM {name} ORDER BY ID")
        cases[-1]["parameter"] = utf16("a ")
    return {"format": FORMAT, "locale": ";LANGID=0x0409;CP=1252;COUNTRY=0",
            "max_rows": MAX_ROWS, "tables": tables, "cases": cases}


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
    kind = cell.get("kind")
    if kind == "null":
        return cell == {"kind": "null"}
    if set(cell) != {"kind", "value"}:
        return False
    value = cell["value"]
    if kind == "text":
        return isinstance(value, str) and HEX.fullmatch(value) is not None
    if kind == "integer":
        return isinstance(value, str) and re.fullmatch(r"-?(0|[1-9][0-9]*)", value) is not None
    if kind == "boolean":
        return type(value) is bool
    return False


def check_shape(case, table):
    """Check coverage/serialization without predicting native text comparisons."""
    suffix = case["id"].split(".")[1]
    rows = case["rows"]
    count = len(table["values"])
    integer = lambda n: {"kind": "integer", "value": str(n)}
    if suffix == "pairs":
        require(case["columns"] == ["LID", "RID", "Eq", "Lt", "Gt"], "invalid pair columns")
        require([r[:2] for r in rows] == [[integer(a), integer(b)] for a in range(count) for b in range(count)], "incomplete pair coverage")
        require(all(c["kind"] in ["boolean", "null"] for r in rows for c in r[2:]), "invalid comparison result types")
    elif suffix.startswith("sort_"):
        require(case["columns"] == ["ID"] and sorted(rows, key=lambda r: int(r[0].get("value", "-1"))) == [[integer(i)] for i in range(count)], "incomplete sorted IDs")
    elif suffix.startswith("like_") or suffix == "parameter_text":
        expected_column = "Matched" if suffix.startswith("like_") else "Eq"
        require(case["columns"] == ["ID", expected_column] and [r[:1] for r in rows] == [[integer(i)] for i in range(count)], "incomplete predicate IDs")
        require(all(r[1]["kind"] in ["boolean", "null"] for r in rows), "invalid predicate result types")
    elif suffix == "join":
        require(case["columns"] == ["LID", "RID"], "invalid join columns")
        pairs = [tuple(int(c.get("value", "-1")) if c["kind"] == "integer" else -1 for c in r) for r in rows]
        require(pairs == sorted(set(pairs)) and all(0 <= i < count for pair in pairs for i in pair), "invalid join IDs")
    elif suffix == "group":
        require(case["columns"] == ["FirstID", "N"] and rows, "invalid grouping columns/rows")
        require(all(c["kind"] == "integer" for r in rows for c in r), "invalid group types")
        ids = [int(r[0]["value"]) for r in rows]
        sizes = [int(r[1]["value"]) for r in rows]
        require(ids == sorted(set(ids)) and all(0 <= i < count for i in ids) and all(n > 0 for n in sizes) and sum(sizes) == count, "incomplete grouping")
    elif suffix == "distinct":
        require(case["columns"] == ["V"] and 1 <= len(rows) <= count and all(r[0]["kind"] in ["text", "null"] for r in rows), "invalid DISTINCT result")


def validate(directory):
    require((directory / "request.json").read_bytes() == request_bytes(), "request differs from the original versioned recipe")
    capture = directory / "capture"
    result = json.loads((capture / "results.json").read_text(encoding="utf-8-sig"))
    require(result.get("format") == FORMAT, "unknown result format")
    require(result.get("request_sha256") == hashlib.sha256(request_bytes()).hexdigest(), "request hash mismatch")
    require(result.get("runner_sha256") == hashlib.sha256(Path(__file__).with_name("Capture-NativeText.ps1").read_bytes()).hexdigest(), "capture runner differs from reviewed source")
    environment = result.get("environment", {})
    for field in ["engine_version", "engine_dll_version", "windows_version", "powershell_version", "process_bits"]:
        require(bool(environment.get(field)), f"missing environment {field}")
    require(environment["process_bits"] in [32, 64], "invalid process bitness")
    databases = result.get("databases", [])
    require([x.get("id") for x in databases] == ["plain", "indexed"], "missing/duplicate database phase")
    counts = Counter()
    for database in databases:
        phase = database["id"]
        path = capture / (phase + ".accdb")
        raw = path.read_bytes()
        require(database.get("bytes") == len(raw), f"{phase}: length mismatch")
        require(SHA256.fullmatch(database.get("sha256", "")) is not None, f"{phase}: invalid hash")
        require(hashlib.sha256(raw).hexdigest() == database["sha256"], f"{phase}: database hash mismatch")
        require(len(raw) >= 128 and raw[4:19] == b"Standard ACE DB", f"{phase}: expected ACCDB header")
        require(database.get("header_hex") == raw[:128].hex(), f"{phase}: header mismatch")
        require(type(database.get("database_collating_order")) is int, f"{phase}: missing database order")
        require(set(database.get("field_collating_orders", {})) == {"ShortValues", "LongValues"}, f"{phase}: missing field orders")
        require(all(type(x) is int for x in database["field_collating_orders"].values()), f"{phase}: invalid field orders")
        cases = database.get("results", [])
        require([x.get("id") for x in cases] == [x["id"] for x in request()["cases"]], f"{phase}: missing/reordered/duplicate cases")
        for case in cases:
            ident = phase + ":" + case["id"]
            status = case.get("status")
            require(status in ["ok", "error"], f"{ident}: missing outcome")
            if status == "error":
                require(type(case.get("hresult")) is int, f"{ident}: missing HRESULT")
                require(isinstance(case.get("dao_numbers"), list) and all(type(x) is int for x in case["dao_numbers"]), f"{ident}: missing DAO error codes")
                require("rows" not in case, f"{ident}: partial rows on failure")
                counts["errors"] += 1
                continue
            columns, rows = case.get("columns"), case.get("rows")
            require(isinstance(columns, list) and columns and all(isinstance(c, str) for c in columns), f"{ident}: invalid columns")
            require(isinstance(rows, list) and len(rows) <= MAX_ROWS, f"{ident}: invalid row count")
            require(all(isinstance(row, list) and len(row) == len(columns) and all(valid_cell(c) for c in row) for row in rows), f"{ident}: invalid typed rows")
            table = next(t for t in request()["tables"] if case["id"].startswith(t["name"] + "."))
            check_shape(case, table)
            counts["captured"] += 1
        for table in request()["tables"]:
            stored = next(c for c in cases if c["id"] == table["name"] + ".stored")
            expected = [[{"kind": "integer", "value": str(i)}, {"kind": "null"} if value is None else {"kind": "text", "value": value}] for i, value in enumerate(table["values"])]
            require(stored.get("status") == "ok" and stored.get("columns") == ["ID", "V"] and stored.get("rows") == expected, f"{phase}: stored values changed in {table['name']}")
    return counts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for command in ["prepare", "check"]:
        sub.add_parser(command).add_argument("directory", type=Path)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.directory)
        print("Prepared original probes; native execution and compatibility remain unrun.")
    else:
        counts = validate(args.directory)
        print(f"Evidence complete: {counts['captured']} captured results, {counts['errors']} native errors. No TableCraft compatibility comparison performed.")


if __name__ == "__main__":
    main()
